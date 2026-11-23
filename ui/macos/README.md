<!-- This directory is Flutter's macOS runner for the Flutter client (#151). -->

# macOS runner

Flutter generated this runner; Koalino changes only its settings:

- `Runner/Configs/AppInfo.xcconfig`: the app is named Koalino. The bundle identifier stays
  `com.example.noteTakerUi`, like the other platforms' internal IDs, until a release changes them.
- `Runner/*.entitlements`: no App Sandbox, since Koalino is not sold through the Mac App Store and
  must run the owner's Claude Code and Codex tools (#105) and talk to the browser extension's
  host (#125). It declares the microphone and network client and server use (P2P sync).
- `Runner/Info.plist`: the microphone and local-network explanations macOS shows when it asks,
  and the Bonjour service libp2p's mDNS uses (`_p2p._udp`).

The Rust core is not built here: `tools/macos/build-macos.sh` builds it, adds it to the finished
`Koalino.app` with the models, signs the bundle ad hoc, and zips it. The Dart side opens the core
from `Contents/Frameworks` (`lib/infrastructure/core_library.dart`) and the models from
`Contents/Resources/models` (`lib/infrastructure/whisper_model.dart`).
