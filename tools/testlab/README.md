# tools/testlab

Runs Koalino on real Android phones in [Firebase Test Lab](https://firebase.google.com/docs/test-lab)
(#182), to check on hardware the project doesn't own what only the emulator profiles (#181)
covered so far.

## automatic-model.robo.json

A Robo script that checks which Whisper model "Automatic" picks on a phone (#106), and whether the
speed check (#109) confirms it:

1. Opens **Settings → Transcription model**.
2. Taps **Download 488 MB** when Automatic suggests `small` (optional: phones under 6 GB are
   suggested the shipped `base` and show no button).
3. Waits ten minutes. Test Lab phones are on USB power, so they count as charging, and the speed
   check times the clip with Automatic's model, then with smaller ones while it falls behind.
4. Reopens the page, whose screenshot shows Automatic's model and the check's result
   ("Checked on this device: 0.4× real time…").

Robo then explores the app on its own until the time limit; only the screenshots up to step 4
matter. Its exploration taps rows too: on the Galaxy A54 it chose "Suggested for this device"
before the reopened page was captured, which hides the check's result (shown on the Automatic
row only). Read the screenshots in time order and use the first reopened page. It taps elements by their accessibility labels (Flutter's semantics), so a renamed tab or
row needs the script updated.

Run it (needs `gcloud auth login` and the `koalino-test-lab` project). Use the phone build
(`flutter build apk --target-platform android-arm64`), never the emulator APK: that one carries
only the x86_64 Flutter engine, and every real phone crashes at launch ("Could not find
'libflutter.so'"):

```bash
gcloud firebase test android run --type robo --app koalino.apk \
  --robo-script tools/testlab/automatic-model.robo.json \
  --device model=a06,version=35 --device model=a54x,version=34 --device model=akita,version=35 \
  --timeout 20m
```

List phone models with `gcloud firebase test android models list --filter=form=PHYSICAL`.
Results, screenshots and logcat land in the project's Test Lab bucket. The free tier allows only
a few physical-device runs a day.
