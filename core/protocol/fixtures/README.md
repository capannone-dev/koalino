# Golden protocol frames (`core/protocol/fixtures`)

Each `<name>.textproto` is a human-readable `TeamsBridgeMessage`. `<name>.bin` is the same
message encoded by `protoc`, an encoder independent of rust-protobuf:

```sh
protoc --proto_path=core/protocol --encode=note_taker.teams_bridge.v1.TeamsBridgeMessage \
  core/protocol/teams_bridge.proto < core/protocol/fixtures/audio.textproto > core/protocol/fixtures/audio.bin
```

`adapters/network/teams_bridge/message_decoder_tests.rs` decodes the `.bin` files and checks the
resulting domain values. The fixtures contain only made-up participants and a few bytes of audio
and image data.
