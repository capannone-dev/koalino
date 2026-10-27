<!-- Explains the screen-share processing stage of the Teams bot. -->

# Video

`VideoProcessor` turns VBSS screen-share frames (NV12, any stride) into JPEG slides:

1. It samples frames at most `Fps` times per second.
2. It skips frames whose 32×18 luma/chroma fingerprint is unchanged.
3. It converts with BT.601 and encodes with SkiaSharp at `JpegQuality`.
4. It re-encodes or downscales any frame that would exceed the 8 MiB message cap.
