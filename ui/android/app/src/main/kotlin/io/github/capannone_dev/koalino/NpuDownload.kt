// Serves `note_taker/npu` (#75): downloads what Whisper needs on a Snapdragon NPU, only when the
// owner asks, so the APK carries no Qualcomm code. Two pinned archives are fetched over HTTPS and
// checked against their SHA-256 before anything is extracted:
//   - Qualcomm's QNN runtime 2.50.0 (Maven Central), from which the HTP (NPU) libraries for the
//     phone's Hexagon version are kept;
//   - Qualcomm AI Hub's precompiled Whisper Small (w8a16) for the phone's chip. The core's chip
//     table (#183) names it, with its hash, size and Hexagon version; the app passes it here.
// Files are extracted into a staging folder and moved into `<npu>/qnn` and `<npu>/model` only when
// both archives are complete, so the core never sees a partial install. The archives are deleted
// afterwards. `ADSP_LIBRARY_PATH` must name `<npu>/qnn` before the core opens the NPU, so the
// NPU can load its skeleton library: `prepareEnvironment` sets it at start-up.
package io.github.capannone_dev.koalino

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.system.Os
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.io.File
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.security.MessageDigest
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicBoolean
import java.util.zip.ZipInputStream
import javax.net.ssl.HttpsURLConnection

/** Downloads, verifies and installs the NPU files. */
class NpuDownload : MethodChannel.MethodCallHandler {
    /** One download at a time, off the main thread. */
    private val executor = Executors.newSingleThreadExecutor()
    private val mainThread = Handler(Looper.getMainLooper())
    private val cancelled = AtomicBoolean(false)

    /** What `progress` reports; written by the download thread. */
    @Volatile private var state = "idle"
    @Volatile private var receivedBytes = 0L
    @Volatile private var error: String? = null
    /** The archives of the running or latest download. */
    @Volatile private var archives: List<Archive> = emptyList()

    /**
     * Answers `download(directory, model…, hexagon)`, `progress()`, `cancel()` and
     * `remove(directory)`.
     *
     * Inputs: `directory` is the core's NPU folder (`<data directory>/npu`); `modelUrl`,
     * `modelSha256`, `modelSizeBytes`, `modelEntryPrefix` and `hexagon` describe the build for
     * this chip, from the core's table. A model outside Qualcomm AI Hub's bucket is refused.
     * Outputs: `download` and `cancel` answer null at once; `progress` answers
     * `{state, receivedBytes, totalBytes, error}` with state `idle`, `downloading`, `done`,
     * `cancelled` or `failed` (error `no_network`, `server`, `corrupted` or `storage`); `remove`
     * answers whether the folder is gone. Side effects: Network and file writes on a background
     * thread; up to about 360 MB downloaded and 460 MB stored.
     */
    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "download" -> {
                val directory = call.argument<String>("directory")
                val wanted = archivesFor(call)
                if (directory == null || wanted == null) {
                    result.error("invalid_argument", "directory and a valid build are required", null)
                    return
                }
                if (state == "downloading") {
                    result.error("busy", "a download is running", null)
                    return
                }
                cancelled.set(false)
                state = "downloading"
                receivedBytes = 0L
                error = null
                archives = wanted
                executor.execute { run(File(directory), wanted) }
                result.success(null)
            }
            "progress" -> result.success(
                mapOf(
                    "state" to state,
                    "receivedBytes" to receivedBytes,
                    "totalBytes" to archives.sumOf { it.sizeBytes },
                    "error" to error,
                ),
            )
            "cancel" -> {
                cancelled.set(true)
                result.success(null)
            }
            "remove" -> {
                val directory = call.argument<String>("directory")
                if (directory == null || state == "downloading") {
                    result.success(false)
                    return
                }
                executor.execute {
                    val folder = File(directory)
                    folder.deleteRecursively()
                    mainThread.post { result.success(!folder.exists()) }
                }
            }
            else -> result.notImplemented()
        }
    }

    /** Downloads both archives, then installs them; records the outcome in `state`. */
    private fun run(directory: File, archives: List<Archive>) {
        val workFolder = File(directory, ".download")
        val staging = File(directory, ".staging")
        try {
            workFolder.deleteRecursively()
            staging.deleteRecursively()
            if (!workFolder.mkdirs() || workFolder.usableSpace < REQUIRED_FREE_BYTES) {
                throw StorageFailure()
            }
            var finishedBytes = 0L
            for (archive in archives) {
                val file = File(workFolder, archive.fileName)
                fetch(archive, file, finishedBytes)
                finishedBytes += archive.sizeBytes
                extract(file, archive, File(staging, archive.folder))
                file.delete()
            }
            for (archive in archives) {
                val target = File(directory, archive.folder)
                target.deleteRecursively()
                if (!File(staging, archive.folder).renameTo(target)) {
                    throw StorageFailure()
                }
            }
            state = "done"
        } catch (_: Cancelled) {
            state = "cancelled"
        } catch (_: Corrupted) {
            error = "corrupted"
            state = "failed"
        } catch (_: ServerFailure) {
            error = "server"
            state = "failed"
        } catch (_: StorageFailure) {
            error = "storage"
            state = "failed"
        } catch (_: IOException) {
            error = "no_network"
            state = "failed"
        } finally {
            workFolder.deleteRecursively()
            staging.deleteRecursively()
        }
    }

    /**
     * Downloads one archive to `file` and checks its size and SHA-256.
     *
     * Side effects: Writes `file`; updates `receivedBytes` from `alreadyReceived`. Throws
     * `Cancelled`, `ServerFailure`, `Corrupted` or an `IOException` from the network.
     */
    private fun fetch(archive: Archive, file: File, alreadyReceived: Long) {
        val connection = URL(archive.url).openConnection() as HttpURLConnection
        if (connection !is HttpsURLConnection) {
            throw ServerFailure()
        }
        connection.connectTimeout = TIMEOUT_MILLIS
        connection.readTimeout = TIMEOUT_MILLIS
        try {
            if (connection.responseCode != HttpURLConnection.HTTP_OK) {
                throw ServerFailure()
            }
            val digest = MessageDigest.getInstance("SHA-256")
            var received = 0L
            connection.inputStream.use { input ->
                writing(file) { output ->
                    val buffer = ByteArray(BUFFER_SIZE)
                    while (true) {
                        if (cancelled.get()) {
                            throw Cancelled()
                        }
                        val count = input.read(buffer)
                        if (count < 0) {
                            break
                        }
                        received += count
                        if (received > archive.sizeBytes) {
                            throw Corrupted()
                        }
                        digest.update(buffer, 0, count)
                        output.write(buffer, 0, count)
                        receivedBytes = alreadyReceived + received
                    }
                }
            }
            val hash = digest.digest().joinToString("") { "%02x".format(it) }
            if (received != archive.sizeBytes || hash != archive.sha256) {
                throw Corrupted()
            }
        } finally {
            connection.disconnect()
        }
    }

    /**
     * Extracts the archive's wanted entries, flattened, into `target`.
     *
     * Side effects: Writes files; throws `Corrupted` when a wanted entry is missing.
     */
    private fun extract(file: File, archive: Archive, target: File) {
        if (!target.mkdirs() && !target.isDirectory) {
            throw StorageFailure()
        }
        val found = mutableSetOf<String>()
        ZipInputStream(file.inputStream().buffered()).use { zip ->
            while (true) {
                val entry = zip.nextEntry ?: break
                val name = entry.name.substringAfterLast('/')
                val wanted = !entry.isDirectory && name in archive.entries &&
                    entry.name.startsWith(archive.entryPrefix)
                if (wanted) {
                    writing(File(target, name)) { output -> zip.copyTo(output, BUFFER_SIZE) }
                    found += name
                }
                if (cancelled.get()) {
                    throw Cancelled()
                }
            }
        }
        if (found != archive.entries) {
            throw Corrupted()
        }
    }

    /** Opens `file` for writing, turning a storage error into `StorageFailure`. */
    private fun writing(file: File, write: (java.io.OutputStream) -> Unit) {
        val output = try {
            file.outputStream()
        } catch (_: IOException) {
            throw StorageFailure()
        }
        output.use(write)
    }

    /** One pinned archive and what is kept from it. */
    private class Archive(
        val url: String,
        val sha256: String,
        val sizeBytes: Long,
        val fileName: String,
        /** `qnn` or `model`, under the NPU folder. */
        val folder: String,
        /** Only entries under this path are kept. */
        val entryPrefix: String,
        val entries: Set<String>,
    )

    private class Cancelled : IOException()
    private class Corrupted : IOException()
    private class ServerFailure : IOException()
    private class StorageFailure : IOException()

    companion object {
        private const val BUFFER_SIZE = 1 shl 16
        private const val TIMEOUT_MILLIS = 30_000

        /** The largest archive plus everything installed, with room to spare. */
        private const val REQUIRED_FREE_BYTES = 800L * 1024 * 1024

        /** Where AI Hub's precompiled models live; a model from anywhere else is refused. */
        private const val AI_HUB_PREFIX =
            "https://qaihub-public-assets.s3.us-west-2.amazonaws.com/qai-hub-models/"

        /** The Hexagon versions whose libraries the QNN runtime below carries. */
        private val HEXAGON_VERSIONS = setOf(68, 69, 73, 75, 79, 81)

        /**
         * Builds the two archives for a call: the QNN runtime, keeping the chip's library pair, and
         * the chip's model.
         *
         * Inputs: The `download` call's arguments. Outputs: The archives, or `null` when an argument
         * is missing or not plausible. Side effects: None.
         */
        private fun archivesFor(call: MethodCall): List<Archive>? {
            val url = call.argument<String>("modelUrl") ?: return null
            val sha256 = call.argument<String>("modelSha256") ?: return null
            val size = call.argument<Number>("modelSizeBytes")?.toLong() ?: return null
            val prefix = call.argument<String>("modelEntryPrefix") ?: return null
            val hexagon = call.argument<Int>("hexagon") ?: return null
            val plausible = url.startsWith(AI_HUB_PREFIX) && url.endsWith(".zip") &&
                sha256.matches(Regex("[0-9a-f]{64}")) && size in 1..1_000_000_000L &&
                prefix.endsWith("/") && !prefix.contains("..") && hexagon in HEXAGON_VERSIONS
            if (!plausible) {
                return null
            }
            return listOf(
                Archive(
                    url = "https://repo1.maven.org/maven2/com/qualcomm/qti/qnn-runtime/2.50.0/" +
                        "qnn-runtime-2.50.0.aar",
                    sha256 = "b507656e4031d8aa6a7ab0fecf725b941096737ad7440ad7ea288f7c9cc9d743",
                    sizeBytes = 71_270_746L,
                    fileName = "qnn-runtime.aar",
                    folder = "qnn",
                    entryPrefix = "jni/arm64-v8a/",
                    entries = setOf(
                        "libQnnHtp.so",
                        "libQnnSystem.so",
                        "libQnnHtpPrepare.so",
                        "libQnnHtpV${hexagon}Stub.so",
                        "libQnnHtpV${hexagon}Skel.so",
                    ),
                ),
                Archive(
                    url = url,
                    sha256 = sha256,
                    sizeBytes = size,
                    fileName = "whisper-small-npu.zip",
                    folder = "model",
                    entryPrefix = prefix,
                    entries = setOf(
                        "encoder.onnx",
                        "encoder.bin",
                        "decoder.onnx",
                        "decoder.bin",
                        "metadata.json",
                    ),
                ),
            )
        }

        /**
         * Points the NPU's library search at the downloaded QNN folder.
         *
         * Inputs: The app context. Side effects: Sets `ADSP_LIBRARY_PATH` for this process, keeping
         * the system's own folders after ours; harmless when nothing is downloaded.
         */
        fun prepareEnvironment(context: Context) {
            val qnnFolder = File(context.filesDir, "note_taker/npu/qnn")
            val systemFolders = "/vendor/lib/rfsa/adsp;/vendor/dsp/cdsp;/system/lib/rfsa/adsp;/dsp"
            try {
                Os.setenv("ADSP_LIBRARY_PATH", "${qnnFolder.path};$systemFolders", true)
            } catch (_: android.system.ErrnoException) {
                // The NPU then cannot find its library; the core falls back to whisper.cpp.
            }
        }
    }
}
