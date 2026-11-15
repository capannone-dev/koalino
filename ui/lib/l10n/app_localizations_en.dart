// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for English (`en`).
class AppLocalizationsEn extends AppLocalizations {
  AppLocalizationsEn([String locale = 'en']) : super(locale);

  @override
  String get unableToCheckTheAppLock =>
      'Unable to check the app lock. Try again.';

  @override
  String get unableToShowTheUnlockPrompt =>
      'Unable to show the unlock prompt. Try again.';

  @override
  String get setAScreenLockOnThis => 'Set a screen lock on this device first.';

  @override
  String get unableToChangeTheAppLock =>
      'Unable to change the app lock. Try again.';

  @override
  String get aCallApp => 'A call app';

  @override
  String get call => 'Call';

  @override
  String call2(Object app) {
    return '$app call';
  }

  @override
  String theCallEndedSoTheRecording(Object tiedApp) {
    return 'The $tiedApp call ended, so the recording stopped.';
  }

  @override
  String get restartYourBrowserAndCallApps =>
      'Restart your browser and call apps once, so Koalino can read who is speaking in calls.';

  @override
  String get unableToLoadDevices => 'Unable to load devices';

  @override
  String get thisDeviceIsNotOnA =>
      'This device is not on a network yet. Connect it and try again.';

  @override
  String get thatDidNotWorkTryAgain => 'That did not work. Try again.';

  @override
  String get use1To64CharactersWithout =>
      'Use 1 to 64 characters, without line breaks.';

  @override
  String get theNameCouldNotBeSaved =>
      'The name could not be saved. Try again.';

  @override
  String get thisInviteHasExpiredMakeA =>
      'This invite has expired. Make a new one on the other device.';

  @override
  String get thisInviteWasMadeOnThis =>
      'This invite was made on this device. Use it on the other one.';

  @override
  String get thatDeviceIsAlreadyPaired => 'That device is already paired.';

  @override
  String get thatIsNotAValidInvite =>
      'That is not a valid invite. Copy it again.';

  @override
  String get deviceSyncIsNotRunning => 'Device sync is not running.';

  @override
  String get theOtherDeviceIsNoLonger =>
      'The other device is no longer pairing. Try again.';

  @override
  String get theOtherAppCouldNotBe => 'The other app could not be opened.';

  @override
  String get theOtherApp => 'the other app';

  @override
  String app(Object displayName) {
    return '$displayName app';
  }

  @override
  String get unableToChangeTheLogLevel => 'Unable to change the log level';

  @override
  String logsExportedTo(Object filePath) {
    return 'Logs exported to $filePath';
  }

  @override
  String get unableToShareTheLogs => 'Unable to share the logs';

  @override
  String get noLogsToExportYet => 'No logs to export yet';

  @override
  String get unableToExportTheLogs => 'Unable to export the logs';

  @override
  String couldNotBeAttached(Object attached, Object failedPaths) {
    return '$attached; $failedPaths could not be attached';
  }

  @override
  String get theFilesCouldNotBeAttached => 'The files could not be attached.';

  @override
  String get unableToShareTheExport => 'Unable to share the export';

  @override
  String exportedTo(Object filePath) {
    return 'Exported to $filePath';
  }

  @override
  String get exportSaved => 'Export saved';

  @override
  String get unableToSaveTheExport => 'Unable to save the export';

  @override
  String get thisMeetingNoLongerExists => 'This meeting no longer exists';

  @override
  String get exportIsUnavailableUntilTheApp =>
      'Export is unavailable until the app has started';

  @override
  String get unableToExportTheMeeting => 'Unable to export the meeting';

  @override
  String get unableToLoadMeetings => 'Unable to load meetings';

  @override
  String get unableToLoadThisMeeting => 'Unable to load this meeting';

  @override
  String get enterATitle => 'Enter a title.';

  @override
  String titlesCanBeAtMostCharacters(
      Object maxMeetingTitleCharacters, Object runes) {
    return 'Titles can be at most $maxMeetingTitleCharacters characters (you have $runes).';
  }

  @override
  String get titlesCanTContainLineBreaks =>
      'Titles can\'t contain line breaks or control characters.';

  @override
  String get notPairedYetMakeTheRelay =>
      'Not paired yet. Make the relay discoverable, then pair with it under Nearby devices in Devices.';

  @override
  String get connectingToTheRelay => 'Connecting to the relay…';

  @override
  String get connectedToTheRelayAskingIt =>
      'Connected to the relay. Asking it to accept connections…';

  @override
  String get connectedYourOtherDevicesCanReach =>
      'Connected. Your other devices can reach this one through the relay.';

  @override
  String get savedThisAppliesNowWithNo =>
      'Saved. This applies now, with no restart.';

  @override
  String get thatRelayAddressIsNotValid =>
      'That relay address is not valid. It looks like /ip4/203.0.113.5/tcp/4001/p2p/12D3Koo…';

  @override
  String get aDeviceAddressIsNotValid =>
      'A device address is not valid. Each ends in /p2p/ and the device\'s ID.';

  @override
  String get unableToSaveTheAddresses => 'Unable to save the addresses.';

  @override
  String get thePhotoCouldNotBeSaved => 'The photo could not be saved.';

  @override
  String get enterAName => 'Enter a name.';

  @override
  String namesCanBeAtMostCharacters(Object maxMeetingTitleCharacters) {
    return 'Names can be at most $maxMeetingTitleCharacters characters.';
  }

  @override
  String get namesCanTContainLineBreaks =>
      'Names can\'t contain line breaks or control characters.';

  @override
  String get unableToLoadProjects => 'Unable to load projects';

  @override
  String get unableToLoadThisProject => 'Unable to load this project';

  @override
  String get theFilesCouldNotBeAdded => 'The files could not be added.';

  @override
  String couldNotBeAdded(Object failedPaths) {
    return '$failedPaths could not be added';
  }

  @override
  String get unableToStartRecording => 'Unable to start recording';

  @override
  String get unableToPauseRecording => 'Unable to pause recording';

  @override
  String get unableToResumeRecording => 'Unable to resume recording';

  @override
  String get unableToStopRecording => 'Unable to stop recording';

  @override
  String get recordingSaved => 'Recording saved';

  @override
  String get microphoneAccessIsNeededToRecord =>
      'Microphone access is needed to record';

  @override
  String get keepTheAppOpenAndroidDid =>
      'Keep the app open: Android did not allow recording in the background';

  @override
  String get unableToOpenTheMicrophone => 'Unable to open the microphone';

  @override
  String saved(Object label) {
    return 'Saved $label.';
  }

  @override
  String removed(Object label) {
    return 'Removed $label.';
  }

  @override
  String isNowTheDefault(Object id) {
    return '$id is now the default.';
  }

  @override
  String answered(Object id, Object reply) {
    return '$id answered: $reply';
  }

  @override
  String get promptSaved => 'Prompt saved.';

  @override
  String get theBuiltInPromptIsBack => 'The built-in prompt is back.';

  @override
  String get finishedMeetingsWillBeSummarizedAutomatically =>
      'Finished meetings will be summarized automatically.';

  @override
  String get automaticSummariesAreOff => 'Automatic summaries are off.';

  @override
  String get thisComputerNoLongerWritesSummaries =>
      'This computer no longer writes summaries or answers chats for your other devices.';

  @override
  String nowWritesTheSummariesAndAnswers(Object id) {
    return '$id now writes the summaries and answers the chats your other devices ask for.';
  }

  @override
  String get requestCancelled => 'Request cancelled.';

  @override
  String get thisMeetingUsesTheSettingsPrompt =>
      'This meeting uses the Settings prompt again.';

  @override
  String get promptSavedForThisMeeting => 'Prompt saved for this meeting.';

  @override
  String get summarySaved => 'Summary saved.';

  @override
  String get theConnection => 'The connection';

  @override
  String get summaryWritten => 'Summary written.';

  @override
  String theTranscriptWasReadInParts(Object parts) {
    return ' The transcript was read in $parts parts.';
  }

  @override
  String leftOut(Object sent, Object imagesSkipped) {
    return ' $sent, $imagesSkipped left out.';
  }

  @override
  String get theModelStoppedBeforeTheEnd =>
      ' The model stopped before the end.';

  @override
  String get waitingForYourComputerItWrites =>
      'Waiting for your computer. It writes the summary once it is on, connected to this device, and set to answer in Settings → AI connections.';

  @override
  String get yourComputerIsWritingTheSummary =>
      'Your computer is writing the summary…';

  @override
  String yourComputerCouldNotWriteThe(Object message) {
    return 'Your computer could not write the summary: $message';
  }

  @override
  String get unableToRefreshTelemetry => 'Unable to refresh telemetry';

  @override
  String get automatic => 'Automatic';

  @override
  String get transcriptionLanguage => 'Transcription language';

  @override
  String get transcriptionLanguages => 'Transcription languages';

  @override
  String get theLanguageCouldNotBeChanged =>
      'The language could not be changed.';

  @override
  String get thisDevice => 'This device';

  @override
  String get bestAvailable => 'Best available';

  @override
  String get aRemovedDevice => 'A removed device';

  @override
  String get theTranscriptionDeviceCouldNotBe =>
      'The transcription device could not be changed. Check that it is still paired.';

  @override
  String get downloadFinishedYouCanNowChoose =>
      'Download finished. You can now choose this model.';

  @override
  String get downloadCancelled => 'Download cancelled.';

  @override
  String get theDownloadStoppedNoConnectionTo =>
      'The download stopped: no connection to the server. Your current model still works.';

  @override
  String get theDownloadedFileFailedItsCheck =>
      'The downloaded file failed its check and was deleted. Your current model still works.';

  @override
  String get theModelCouldNotBeSaved =>
      'The model could not be saved. Check the free space. Your current model still works.';

  @override
  String get theDownloadServerRefusedTheFile =>
      'The download server refused the file. Your current model still works.';

  @override
  String get savedTheNextRecordingUsesThis =>
      'Saved. The next recording uses this model.';

  @override
  String get savedTheNextRecordingUsesThis2 =>
      'Saved. The next recording uses this.';

  @override
  String get savedTheNextRecordingUsesThe =>
      'Saved. The next recording uses the automatic choice.';

  @override
  String get savedMeetingsRecordedFromNowOn =>
      'Saved. Meetings recorded from now on use this.';

  @override
  String get savedTheNextRecordingUsesThese =>
      'Saved. The next recording uses these names and terms.';

  @override
  String get eachTermCanHaveAtMost =>
      'Each term can have at most 60 characters, and all of them together at most 500.';

  @override
  String get chooseAWindowBetween1And =>
      'Choose a window between 1 and 60 days.';

  @override
  String get theDownloadCouldNotStartPlease =>
      'The download could not start. Please try again.';

  @override
  String get savedTheNextRecordingIsTranscribed =>
      'Saved. The next recording is transcribed on the NPU.';

  @override
  String get savedTheNextRecordingIsTranscribed2 =>
      'Saved. The next recording is transcribed on the processor.';

  @override
  String get theNpuFilesAreNotAll =>
      'The NPU files are not all downloaded yet.';

  @override
  String get theNpuFilesWereRemoved => 'The NPU files were removed.';

  @override
  String get theNpuFilesCouldNotBe =>
      'The NPU files could not be removed. Please try again.';

  @override
  String get downloadedRecordingsNowTranscribeOnThe =>
      'Downloaded. Recordings now transcribe on the NPU.';

  @override
  String get theDownloadStoppedNoConnectionTo2 =>
      'The download stopped: no connection to the server.';

  @override
  String get aDownloadedFileFailedItsCheck =>
      'A downloaded file failed its check and was deleted.';

  @override
  String get theFilesCouldNotBeSaved =>
      'The files could not be saved. Check the free space (about 800 MB is needed).';

  @override
  String get theDownloadServerRefusedTheFiles =>
      'The download server refused the files.';

  @override
  String get theNextRecordingTriesTheGraphics =>
      'The next recording tries the graphics chip again.';

  @override
  String get thatModelWonTBeProposed => 'That model won\'t be proposed again.';

  @override
  String get modelRemoved => 'Model removed.';

  @override
  String get anotherModelIsDownloadingWaitFor =>
      'Another model is downloading. Wait for it to finish.';

  @override
  String get thereIsnTEnoughFreeSpace =>
      'There isn\'t enough free space for this model.';

  @override
  String get thatDidnTWorkPleaseTry => 'That didn\'t work. Please try again.';

  @override
  String get unableToLoadVoiceGallery => 'Unable to load Voice Gallery';

  @override
  String get unableToPlayThisVoice => 'Unable to play this voice';

  @override
  String get unableToDeleteThisVoice => 'Unable to delete this voice';

  @override
  String get voiceDeleted => 'Voice deleted';

  @override
  String get unableToDeleteThisPerson => 'Unable to delete this person';

  @override
  String deleted(Object displayName) {
    return '$displayName deleted';
  }

  @override
  String get enterANameBeforeAssigning => 'Enter a name before assigning';

  @override
  String assignedTo(Object identityName) {
    return 'Assigned to $identityName';
  }

  @override
  String assignedToThereWasNotEnough(Object identityName) {
    return 'Assigned to $identityName. There was not enough clear speech to recognize this voice in future recordings.';
  }

  @override
  String get unableToAssignTheSpeakerCluster =>
      'Unable to assign the speaker cluster';

  @override
  String get chooseAtLeastOneVoiceTo => 'Choose at least one voice to name';

  @override
  String thereWasNotEnoughClearSpeech(Object assigned) {
    return '$assigned. There was not enough clear speech to recognize this voice in future recordings.';
  }

  @override
  String get theSettingCouldNotBeSaved => 'The setting could not be saved.';

  @override
  String get theAppIsStillStartingTry =>
      'The app is still starting. Try again.';

  @override
  String get somethingWentWrongTryAgain => 'Something went wrong. Try again.';

  @override
  String isUsingTheMicrophone(Object app) {
    return '$app is using the microphone';
  }

  @override
  String get recordThisCall => 'Record this call?';

  @override
  String theAiServerRejectedTheRequest(Object detail) {
    return 'The AI server rejected the request.\nThe server said: $detail';
  }

  @override
  String claudeCodeOrCodexCouldNot(Object detail) {
    return 'Claude Code or Codex could not answer.\nIt said: $detail';
  }

  @override
  String get typeAQuestion => 'Type a question.';

  @override
  String get theQuestionIsTooLong => 'The question is too long.';

  @override
  String get addAnAiConnectionInSettings =>
      'Add an AI connection in Settings, or choose a default one.';

  @override
  String get thatAiConnectionNoLongerExists =>
      'That AI connection no longer exists.';

  @override
  String get thereIsNothingToTalkAbout =>
      'There is nothing to talk about yet: no transcript, summary, notes or files.';

  @override
  String get theAiConnectionSContextIs =>
      'The AI connection\'s context is too small. Choose one with a larger context.';

  @override
  String get thatMeetingNoLongerExists => 'That meeting no longer exists.';

  @override
  String get thatProjectNoLongerExists => 'That project no longer exists.';

  @override
  String get theAiServerCouldNotBe =>
      'The AI server could not be reached. Check the connection.';

  @override
  String get theAiServerRefusedTheApi => 'The AI server refused the API key.';

  @override
  String get theAiServerIsBusyTry => 'The AI server is busy. Try again later.';

  @override
  String get theModelDeclinedToAnswer => 'The model declined to answer.';

  @override
  String get claudeCodeOrCodexIsNot =>
      'Claude Code or Codex is not installed on this computer, or the app cannot find it.';

  @override
  String get theNotesCouldNotBeRead => 'The notes could not be read.';

  @override
  String get noPairedComputerAnsweredOnA =>
      'No paired computer answered. On a computer, open Settings → AI and choose a connection under \"Summaries for my other devices\", then keep it on and connected.';

  @override
  String get theComputerTookTooLongTo =>
      'The computer took too long to answer.';

  @override
  String get theComputerDisconnectedBeforeAnsweringTry =>
      'The computer disconnected before answering. Try again.';

  @override
  String get theComputerAnswersAtMost30 =>
      'The computer answers at most 30 questions an hour. Try again later.';

  @override
  String get theComputerIsAnsweringOtherQuestions =>
      'The computer is answering other questions. Try again shortly.';

  @override
  String get pairedDevicesAreOffOnThis =>
      'Paired devices are off on this phone.';

  @override
  String get theAiServerCouldNotAnswer =>
      'The AI server could not answer. Try again.';

  @override
  String get exportMeeting => 'Export meeting';

  @override
  String get openaiCompatibleServer => 'OpenAI-compatible server';

  @override
  String get localServerOllama => 'Local server (Ollama)';

  @override
  String get claudeCodeClaudePlan => 'Claude Code (Claude plan)';

  @override
  String get codexChatgptPlan => 'Codex (ChatGPT plan)';

  @override
  String get checkTheAddressAndModelPlain =>
      'Check the address and model. Plain http:// works only on this device or the local network.';

  @override
  String get thatConnectionNoLongerExists =>
      'That connection no longer exists.';

  @override
  String get addAnAiConnectionInSettings2 =>
      'Add an AI connection in Settings first.';

  @override
  String get thisMeetingHasNoTranscriptOr =>
      'This meeting has no transcript or notes yet.';

  @override
  String get thisMeetingIsTooLongFor =>
      'This meeting is too long for this connection. Choose one with a larger context.';

  @override
  String get theAiServerCouldNotBe2 => 'The AI server could not be reached.';

  @override
  String get theAiServerRefusedAccessUse =>
      'The AI server refused access. Use a server that needs no API key.';

  @override
  String get theAiServerIsBusyOr =>
      'The AI server is busy or over its limit. Try again later.';

  @override
  String get theAiServerRejectedTheRequest2 =>
      'The AI server rejected the request. Check the model name.';

  @override
  String get theAiServerFailedTryAgain => 'The AI server failed. Try again.';

  @override
  String get theAiServerSAnswerCould =>
      'The AI server\'s answer could not be read.';

  @override
  String get claudeCodeOrCodexCouldNot2 =>
      'Claude Code or Codex could not answer. Check that it is signed in.';

  @override
  String get theSettingsCouldNotBeSaved => 'The settings could not be saved.';

  @override
  String get theMeetingCouldNotBeRead =>
      'The meeting could not be read or saved.';

  @override
  String get error => 'Error';

  @override
  String get warning => 'Warning';

  @override
  String get infoDefault => 'Info (default)';

  @override
  String get debug => 'Debug';

  @override
  String get trace => 'Trace';

  @override
  String get thisDevice2 => 'this device';

  @override
  String get anotherDevice => 'another device';

  @override
  String notConnected(Object label) {
    return '$label (not connected)';
  }

  @override
  String itIsKeptEncryptedUntil(Object until) {
    return ' It is kept encrypted until $until.';
  }

  @override
  String get recordedOnThisDeviceOnly => 'Recorded on this device only.';

  @override
  String recordedOnThisDeviceOnlyA(Object until) {
    return 'Recorded on this device only. A recording of it made on another device can still be combined until $until.';
  }

  @override
  String combinesTheRecordingsNothingIsLeft(Object combiner) {
    return '$combiner combines the recordings; nothing is left to send from here.';
  }

  @override
  String waitingForAudioFrom(Object names, Object kept) {
    return 'Waiting for audio from $names.$kept';
  }

  @override
  String combinesTheRecordingsWaitingForAudio(Object combiner, Object names) {
    return '$combiner combines the recordings; waiting for audio from $names.';
  }

  @override
  String get audioFromEveryDeviceIsHere =>
      'Audio from every device is here; the transcript is combined when this device is free.';

  @override
  String thisDeviceSAudioStillHas(Object combiner, Object kept) {
    return 'This device\'s audio still has to reach $combiner.$kept';
  }

  @override
  String get transcriptCombinedFromEveryDevice =>
      'Transcript combined from every device.';

  @override
  String notCombinedAudioFromDidNot(Object plainNames) {
    return 'Not combined: audio from $plainNames did not arrive within three weeks.';
  }

  @override
  String meeting(Object createdAt) {
    return 'Meeting $createdAt';
  }

  @override
  String get writeSomethingFirst => 'Write something first.';

  @override
  String get theTextIsTooLong => 'The text is too long.';

  @override
  String get thatPartOfTheTranscriptChanged =>
      'That part of the transcript changed meanwhile. Reopen the meeting.';

  @override
  String get theMeetingCouldNotBeSaved => 'The meeting could not be saved.';

  @override
  String get chooseAFullFolderPathInside =>
      'Choose a full folder path, inside your home folder but not the home folder itself.';

  @override
  String get theFolderCouldNotBeWritten =>
      'The folder could not be written. Check that it exists and you can write there.';

  @override
  String get theNotesCouldNotBeRead2 => 'The notes could not be read or saved.';

  @override
  String get theInviteAnswerIsIncomplete => 'The invite answer is incomplete.';

  @override
  String get theRenameAnswerIsIncomplete => 'The rename answer is incomplete.';

  @override
  String get thisDeviceHasNoCamera => 'This device has no camera.';

  @override
  String get theCameraIsNotOpen => 'The camera is not open.';

  @override
  String get title => 'Title';

  @override
  String get summary => 'Summary';

  @override
  String get notes => 'Notes';

  @override
  String get transcript => 'Transcript';

  @override
  String get projectName => 'Project name';

  @override
  String file(Object fileName) {
    return 'File $fileName';
  }

  @override
  String get typeWhatToLookFor => 'Type what to look for.';

  @override
  String get typeAtLeastTwoLetters => 'Type at least two letters.';

  @override
  String get theSearchIsTooLong => 'The search is too long.';

  @override
  String get exportLogs => 'Export logs';

  @override
  String about(Object appName) {
    return 'About $appName';
  }

  @override
  String get versionLicenseAndTheSoftwareIt =>
      'Version, license and the software it uses.';

  @override
  String remove(Object label) {
    return 'Remove $label?';
  }

  @override
  String get itsApiKeyIsDeletedFrom =>
      'Its API key is deleted from this device. Summaries already written stay.';

  @override
  String get cancel => 'Cancel';

  @override
  String get remove2 => 'Remove';

  @override
  String get aiConnections => 'AI connections';

  @override
  String get addConnection => 'Add connection';

  @override
  String get summariesSendTheMeetingSTranscript =>
      'Summaries send the meeting\'s transcript, notes and photos to the connection\'s server. A server on your local network keeps them there.';

  @override
  String get noConnectionsYet => 'No connections yet';

  @override
  String get addAServerOnYourNetwork =>
      'Add a server on your network (Ollama, LM Studio, llama.cpp), or Claude Code or Codex on this computer.';

  @override
  String get summarizeMeetingsAutomatically =>
      'Summarize meetings automatically';

  @override
  String get withTheDefaultConnectionWhenA =>
      'With the default connection, when a meeting\'s transcript is final.';

  @override
  String get summariesForMyOtherDevices => 'Summaries for my other devices';

  @override
  String get aPairedPhoneCanAskThis =>
      'A paired phone can ask this computer to write a summary or answer a chat with this connection, from this computer\'s copy of your notes. It counts against that plan; at most 10 summaries and 30 chat answers an hour.';

  @override
  String get off => 'Off';

  @override
  String get summaryPrompt => 'Summary prompt';

  @override
  String placeholdersTheTranscriptIsAlwaysSent(Object placeholders) {
    return 'Placeholders: $placeholders. The transcript is always sent after the prompt.';
  }

  @override
  String get savePrompt => 'Save prompt';

  @override
  String get resetToBuiltIn => 'Reset to built-in';

  @override
  String get defaultLabel => 'Default';

  @override
  String get installed => 'Installed';

  @override
  String get notFoundOnThisComputer => 'Not found on this computer';

  @override
  String get unencryptedHttpLocalNetworkOnly =>
      'Unencrypted http, local network only';

  @override
  String get runsWithYourPlanSSign =>
      'Runs with your plan\'s sign-in on this computer';

  @override
  String actionsFor(Object label) {
    return 'Actions for $label';
  }

  @override
  String get makeDefault => 'Make default';

  @override
  String get test => 'Test';

  @override
  String get edit => 'Edit';

  @override
  String get enterANameAnAddressAnd => 'Enter a name, an address and a model.';

  @override
  String get theContextIsANumberOf =>
      'The context is a number of tokens, such as 8192.';

  @override
  String edit2(Object label) {
    return 'Edit $label';
  }

  @override
  String get name => 'Name';

  @override
  String get protocol => 'Protocol';

  @override
  String get runsTheToolInstalledAndSigned =>
      'Runs the tool installed and signed in on this computer, so your Claude or ChatGPT plan writes the summaries instead of API credit. The meeting goes to Anthropic or OpenAI under that account.';

  @override
  String get address => 'Address';

  @override
  String get httpsAlwaysHttpOnlyOnThis =>
      'https:// always; http:// only on this device or the local network.';

  @override
  String get model => 'Model';

  @override
  String get leaveEmptyForTheToolS =>
      'Leave empty for the tool\'s own default.';

  @override
  String get contextTokens => 'Context (tokens)';

  @override
  String get saveConnection => 'Save connection';

  @override
  String get koalinoIsLocked => 'Koalino is locked';

  @override
  String get unlockWithYourFingerprintFaceOr =>
      'Unlock with your fingerprint, face, or screen lock.';

  @override
  String get unlock => 'Unlock';

  @override
  String copied(Object what) {
    return '$what copied';
  }

  @override
  String get aiAssistantAccess => 'AI assistant access';

  @override
  String get letYourOwnAiAssistantSuch =>
      'Let your own AI assistant, such as Claude Code, Codex or Claude Desktop, read your meetings and projects: titles, summaries, notes, transcripts and project files. It can only read; it cannot change or delete anything.';

  @override
  String get whatTheAssistantReadsMayBe =>
      'What the assistant reads may be sent to its online model, under your account with that service. Turn this on only for an assistant you trust with your meetings.';

  @override
  String get theAssistantServerIsNotInstalled =>
      'The assistant server is not installed with this copy of the app.';

  @override
  String get letMyAiAssistantReadMy => 'Let my AI assistant read my notes';

  @override
  String get connectYourAssistant => 'Connect your assistant';

  @override
  String get yourNotesAreUnlockedWithA =>
      'Your notes are unlocked with a passphrase, which is never written into an assistant\'s settings. The server reads them only when NOTE_TAKER_ENCRYPTION_PASSPHRASE is set where the assistant starts it.';

  @override
  String get claudeCodeRunInATerminal => 'Claude Code: run in a terminal';

  @override
  String get command => 'Command';

  @override
  String get claudeDesktopAndOtherMcpClients =>
      'Claude Desktop and other MCP clients: add to their settings';

  @override
  String get settings => 'Settings';

  @override
  String copy(Object title) {
    return 'Copy: $title';
  }

  @override
  String isUsingTheMicrophoneRecordThis(Object app) {
    return '$app is using the microphone. Record this call?';
  }

  @override
  String get notNow => 'Not now';

  @override
  String get record => 'Record';

  @override
  String get offerToRecordCalls => 'Offer to record calls';

  @override
  String get whenTeamsZoomMeetInA =>
      'When Teams, Zoom, Meet in a browser or another call app starts using the microphone. Nothing records until you tap Record.';

  @override
  String get stopWhenTheCallEnds => 'Stop when the call ends';

  @override
  String get onlyForRecordingsStartedFromThe =>
      'Only for recordings started from the offer, once the call app has left the microphone for 20 seconds.';

  @override
  String get readSpeakerNamesFromTheCall =>
      'Read speaker names from the call window';

  @override
  String get whileRecordingKoalinoReadsWhoTeams =>
      'While recording, Koalino reads who Teams, Zoom or Meet shows as speaking, to suggest names for unknown voices. Nothing leaves this computer, and nothing is named until you tap Assign.';

  @override
  String get deleteThisChat => 'Delete this chat?';

  @override
  String get itsQuestionsAndAnswersAreDeleted =>
      'Its questions and answers are deleted from this device.';

  @override
  String get delete => 'Delete';

  @override
  String get thatFileIsNoLongerIn => 'That file is no longer in the project.';

  @override
  String get whoAnswers => 'Who answers';

  @override
  String get myPairedComputer => 'My paired computer';

  @override
  String get thisPhoneSAiConnection => 'This phone\'s AI connection';

  @override
  String get deleteThisChat2 => 'Delete this chat';

  @override
  String get askAboutYourNotesAnswersCome =>
      'Ask about your notes. Answers come from your paired computer, which reads its own copy of your notes with the connection chosen there for your other devices.';

  @override
  String get askAboutYourNotesTheAnswer =>
      'Ask about your notes. The answer comes from your default AI connection, which is sent the notes that fit.';

  @override
  String get yourQuestion => 'Your question';

  @override
  String get ask => 'Ask';

  @override
  String get open => 'Open';

  @override
  String given(Object included) {
    return 'Given: $included';
  }

  @override
  String leftOutForLength(Object omitted) {
    return 'Left out for length: $omitted';
  }

  @override
  String answeredBy(Object connection) {
    return 'Answered by $connection';
  }

  @override
  String get unableToOpenTheKeychain => 'Unable to open the keychain';

  @override
  String get koalinoKeepsTheKeyThatEncrypts =>
      'Koalino keeps the key that encrypts your voice profiles and device identity in the system keychain. Unlock the keychain, or allow Koalino to use it, then try again. If this device has no keychain service, install or start one and restart Koalino.';

  @override
  String get unableToOpenYourNotes => 'Unable to open your notes';

  @override
  String get koalinoCouldNotOpenItsDatabase =>
      'Koalino could not open its database. Check that there is free space and try again.';

  @override
  String get unlockingYourNotes => 'Unlocking your notes…';

  @override
  String get quit => 'Quit';

  @override
  String get retry => 'Retry';

  @override
  String delete2(Object title) {
    return 'Delete $title?';
  }

  @override
  String get itsTranscriptAttachmentsAndUnknownVoices =>
      'Its transcript, attachments and unknown voices go for good, here and on your paired devices. People you saved stay.';

  @override
  String get unableToDeleteThisMeetingIf =>
      'Unable to delete this meeting. If it is being recorded, stop the recording first.';

  @override
  String get devices => 'Devices';

  @override
  String get deviceSyncIsOff => 'Device sync is off';

  @override
  String get meetingsStayOnThisDeviceUntil =>
      'Meetings stay on this device until sync is turned on.';

  @override
  String thisDevice3(Object capabilities) {
    return 'This device\n$capabilities';
  }

  @override
  String get renameThisDevice => 'Rename this device';

  @override
  String get discoverableYourOtherDevicesCanAsk =>
      'Discoverable: your other devices can ask to pair for a few minutes.';

  @override
  String get makeDiscoverable => 'Make discoverable';

  @override
  String get pairingRequests => 'Pairing requests';

  @override
  String get acceptOnlyIfTheOtherDevice =>
      'Accept only if the other device shows the same code.';

  @override
  String get reject => 'Reject';

  @override
  String get accept => 'Accept';

  @override
  String get pairingInProgress => 'Pairing in progress';

  @override
  String get waitingForTheOtherDevice => 'Waiting for the other device…';

  @override
  String get checkThatTheOtherDeviceShows =>
      'Check that the other device shows the same code.';

  @override
  String get cancelPairing => 'Cancel pairing';

  @override
  String get codesMatch => 'Codes match';

  @override
  String get pairedDevices => 'Paired devices';

  @override
  String get noPairedDevicesYet => 'No paired devices yet.';

  @override
  String paired(Object pairedAt, Object hasRelay) {
    return 'Paired $pairedAt\n$hasRelay';
  }

  @override
  String remove3(Object deviceName) {
    return 'Remove $deviceName';
  }

  @override
  String get nearbyDevices => 'Nearby devices';

  @override
  String get pair => 'Pair';

  @override
  String get anotherNetwork => 'Another network';

  @override
  String get showAnInviteHereAndUse =>
      'Show an invite here and use it on the other device. You still compare a code before they pair.';

  @override
  String get showInvite => 'Show invite';

  @override
  String get useInvite => 'Use invite';

  @override
  String stopsSyncingWithThisDevicePair(Object deviceName) {
    return '$deviceName stops syncing with this device. Pair again to resume.';
  }

  @override
  String pairingCode(Object split) {
    return 'Pairing code $split';
  }

  @override
  String get noOtherDevicesFoundOnThis =>
      'No other devices found on this network. Nearby only finds devices on the same Wi-Fi. For a device on another network, use an invite.';

  @override
  String get noOtherDevicesFoundOnThis2 =>
      'No other devices found on this network. Nearby only finds devices on the same Wi-Fi. For a device on another network, use an invite or set a relay in Settings.';

  @override
  String get notConnectedItReconnectsWhenIt =>
      'Not connected. It reconnects when it is online and can reach your relay.';

  @override
  String get notConnectedPutBothDevicesOn =>
      'Not connected. Put both devices on the same Wi-Fi, or set a relay in Settings.';

  @override
  String get whisperOnDevice => 'Whisper (on device)';

  @override
  String get openaiCloud => 'OpenAI (cloud)';

  @override
  String gbGpu(Object round) {
    return '$round GB GPU';
  }

  @override
  String mbGpu(Object gpuMegabytes) {
    return '$gpuMegabytes MB GPU';
  }

  @override
  String get noTranscriptionModels => 'No transcription models';

  @override
  String get useYourOwnAiApp => 'Use your own AI app';

  @override
  String get theMeetingItsNotesAndIts =>
      'The meeting, its notes and its files go to the app you pick, with your own account. For a summary, copy the answer there and come back, or share it to Koalino: Koalino keeps it.';

  @override
  String summarizeIn(Object displayName) {
    return 'Summarize in $displayName';
  }

  @override
  String chatIn(Object displayName) {
    return 'Chat in $displayName';
  }

  @override
  String get summarizeInAnotherApp => 'Summarize in another app…';

  @override
  String get chatInAnotherApp => 'Chat in another app…';

  @override
  String summaryFromSavedTo(Object appName, Object title) {
    return 'Summary from $appName saved to \"$title\".';
  }

  @override
  String get undo => 'Undo';

  @override
  String get theEarlierSummaryIsBack => 'The earlier summary is back.';

  @override
  String toKeepTheSummaryCopyS(Object appName) {
    return 'To keep the summary, copy $appName\'s answer and come back to Koalino, or share the answer to Koalino.';
  }

  @override
  String get meetings => 'Meetings';

  @override
  String get projects => 'Projects';

  @override
  String get voices => 'Voices';

  @override
  String get meeting2 => 'Meeting';

  @override
  String get chatAboutThisMeeting => 'Chat about this meeting';

  @override
  String chat(Object meeting) {
    return 'Chat: $meeting';
  }

  @override
  String get thisMeeting => 'this meeting';

  @override
  String get takePhotos => 'Take photos';

  @override
  String get attachFiles => 'Attach files';

  @override
  String get export => 'Export';

  @override
  String shareAs(Object label) {
    return 'Share as $label';
  }

  @override
  String get nameSpeakers => 'Name speakers';

  @override
  String get selectAMeetingToSeeIts =>
      'Select a meeting to see its transcript.';

  @override
  String get renameMeeting => 'Rename meeting';

  @override
  String get deleteMeeting => 'Delete meeting';

  @override
  String recordedOn(Object devices) {
    return 'Recorded on $devices';
  }

  @override
  String get split => 'Split';

  @override
  String get longPressOrRightClickA =>
      'Long-press or right-click a passage to correct it.';

  @override
  String get noTranscriptIsAvailable => 'No transcript is available.';

  @override
  String get theSpeakerIsAGuess => 'The speaker is a guess';

  @override
  String get peopleTalkedAtOnceHereWords =>
      'People talked at once here; words may be missing';

  @override
  String get overlappingSpeech => 'Overlapping speech';

  @override
  String get someWordsWereUnclear => 'Some words were unclear';

  @override
  String get addedAfterTheMeeting => 'Added after the meeting';

  @override
  String photoOpenFullScreen(Object fileName) {
    return 'Photo $fileName, open full screen';
  }

  @override
  String get save => 'Save';

  @override
  String get preview => 'Preview';

  @override
  String get editNotes => 'Edit notes';

  @override
  String get writeNotes => 'Write notes';

  @override
  String get loading => 'Loading…';

  @override
  String get noNotesYet => 'No notes yet.';

  @override
  String textAt(Object startTimeMs) {
    return 'Text at $startTimeMs';
  }

  @override
  String get removedThisTranscriptWonTBe =>
      'Removed. This transcript won\'t be re-transcribed after the meeting.';

  @override
  String get savedThisTranscriptWonTBe =>
      'Saved. This transcript won\'t be re-transcribed after the meeting.';

  @override
  String get editText => 'Edit text';

  @override
  String get editTheTranscript => 'Edit the transcript';

  @override
  String get leaveItEmptyToRemoveThis => 'Leave it empty to remove this part.';

  @override
  String get searchMeetings => 'Search meetings';

  @override
  String get chatWithAllYourNotes => 'Chat with all your notes';

  @override
  String get chatWithAllNotes => 'Chat with all notes';

  @override
  String get refresh => 'Refresh';

  @override
  String get noMeetingsYet => 'No meetings yet';

  @override
  String get recordingsYouStartAppearHere =>
      'Recordings you start appear here.';

  @override
  String get rename => 'Rename';

  @override
  String get addToProject => 'Add to project…';

  @override
  String get summary2 => 'Summary…';

  @override
  String get addToProject2 => 'Add to project';

  @override
  String writingTheSummaryOf(Object title) {
    return 'Writing the summary of $title';
  }

  @override
  String summaryOf(Object title) {
    return 'Summary of $title';
  }

  @override
  String summarizeWith(Object label) {
    return 'Summarize with $label';
  }

  @override
  String defaultLabel2(Object model) {
    return '$model · default';
  }

  @override
  String get summarizeOnMyComputer => 'Summarize on my computer';

  @override
  String get withClaudeCodeCodexOrThe =>
      'With Claude Code, Codex or the connection set up there';

  @override
  String get useYourOwnAiApp2 => 'Use your own AI app…';

  @override
  String get claudeChatgptOrAnotherAppWith =>
      'Claude, ChatGPT or another app, with your account';

  @override
  String get openSummary => 'Open summary';

  @override
  String get editPromptForThisMeeting => 'Edit prompt for this meeting…';

  @override
  String get useTheSettingsPrompt => 'Use the Settings prompt';

  @override
  String get promptForThisMeeting => 'Prompt for this meeting';

  @override
  String placeholders(Object placeholders) {
    return 'Placeholders: $placeholders';
  }

  @override
  String get addAnAiConnectionInSettings3 =>
      'Add an AI connection in Settings to write summaries.';

  @override
  String get summarizeWith2 => 'Summarize with';

  @override
  String get summarize => 'Summarize';

  @override
  String get summarizeAgain => 'Summarize again';

  @override
  String get editPromptForThisMeeting2 => 'Edit prompt for this meeting';

  @override
  String get writingTheSummaryALocalModel =>
      'Writing the summary… A local model can take a few minutes.';

  @override
  String get noSummaryYet => 'No summary yet.';

  @override
  String get writeASummary => 'Write a summary';

  @override
  String writtenByYouOn(Object createdAt) {
    return 'Written by you on $createdAt';
  }

  @override
  String writtenByOn(
      Object model, Object connectionLabel, Object createdAt, Object edited) {
    return 'Written by $model ($connectionLabel) on $createdAt$edited';
  }

  @override
  String get editSummary => 'Edit summary';

  @override
  String get tryAgainOnMyComputer => 'Try again on my computer';

  @override
  String get cancelRequest => 'Cancel request';

  @override
  String get notesFolder => 'Notes folder';

  @override
  String get keepYourProjectsAndMeetingsAs =>
      'Keep your projects and meetings as plain files in a folder, to read and edit them with any editor. Edits to notes.md, summary.md and project files come back into the app within half a minute, and files you drop into a project folder or a meeting\'s attachments folder are added. transcript.md is rewritten by the app: correct the transcript in the app.';

  @override
  String get theFolderIsADecryptedCopy =>
      'The folder is a decrypted copy of your notes. Anyone who can read it can read them, and backup or sync tools will copy it. Keep it on an encrypted disk, inside your own user folder.';

  @override
  String get folder => 'Folder';

  @override
  String get chooseAFolder => 'Choose a folder';

  @override
  String get keepTheNotesFolder => 'Keep the notes folder';

  @override
  String get syncNow => 'Sync now';

  @override
  String lastSyncAtFailed(Object at, Object code) {
    return 'Last sync at $at failed: $code';
  }

  @override
  String lastSyncAtFilesWrittenEdits(
      Object at, Object written, Object imported) {
    return 'Last sync at $at: $written files written, $imported edits and files taken in.';
  }

  @override
  String get inviteADevice => 'Invite a device';

  @override
  String get pairingInviteQrCode => 'Pairing invite QR code';

  @override
  String onTheOtherDeviceOpenDevices(Object hour, Object minute) {
    return 'On the other device, open Devices, choose Use invite, and paste this link. You can scan the code with a camera app to copy it. It works until $hour:$minute.';
  }

  @override
  String get close => 'Close';

  @override
  String get inviteLinkCopied => 'Invite link copied';

  @override
  String get copyLink => 'Copy link';

  @override
  String get deviceFoundChoosePairUnderNearby =>
      'Device found. Choose Pair under Nearby devices.';

  @override
  String get useAnInvite => 'Use an invite';

  @override
  String get inviteLink => 'Invite link';

  @override
  String get findDevice => 'Find device';

  @override
  String get pairWithAnotherDevice => 'Pair with another device?';

  @override
  String get youOpenedAPairingInviteKoalino =>
      'You opened a pairing invite. Koalino will find the device that shows it, and both devices will show a code to compare before anything is shared.';

  @override
  String get pairingStartedCheckThatBothDevices =>
      'Pairing started. Check that both devices show the same code.';

  @override
  String get done => 'Done';

  @override
  String get photosAreAddedToTheMeeting =>
      'Photos are added to the meeting as you take them.';

  @override
  String get takePhoto => 'Take photo';

  @override
  String get theCameraIsUnavailable => 'The camera is unavailable.';

  @override
  String search(Object name) {
    return 'Search $name';
  }

  @override
  String get noteSaved => 'Note saved';

  @override
  String get theNoteCouldNotBeSaved => 'The note could not be saved.';

  @override
  String couldNotBeAdded2(Object failed) {
    return '$failed could not be added.';
  }

  @override
  String get renameProject => 'Rename project';

  @override
  String get theProjectCouldNotBeRenamed => 'The project could not be renamed.';

  @override
  String delete3(Object project) {
    return 'Delete \"$project\"?';
  }

  @override
  String get itsFilesNotesAndEMails =>
      'Its files, notes and e-mails are deleted on every paired device. Its meetings are kept, outside any project.';

  @override
  String get theProjectCouldNotBeDeleted => 'The project could not be deleted.';

  @override
  String get itIsDeletedOnEveryPaired =>
      'It is deleted on every paired device.';

  @override
  String get theFileCouldNotBeDeleted => 'The file could not be deleted.';

  @override
  String get theMeetingCouldNotBeRemoved =>
      'The meeting could not be removed from the project.';

  @override
  String get project => 'Project';

  @override
  String get searchThisProject => 'Search this project';

  @override
  String get chatAboutThisProject => 'Chat about this project';

  @override
  String get addFiles => 'Add files';

  @override
  String get writeANote => 'Write a note';

  @override
  String get addMeetings => 'Add meetings';

  @override
  String get deleteProject => 'Delete project';

  @override
  String get addThemHereOrFromA => 'Add them here, or from a meeting\'s page.';

  @override
  String get removeFromProject => 'Remove from project';

  @override
  String get filesNotesAndEMails => 'Files, notes and e-mails';

  @override
  String get nothingAddedYet => 'Nothing added yet';

  @override
  String get addPicturesMarkdownPdfsEMails =>
      'Add pictures, Markdown, PDFs, e-mails (.eml) or documents.';

  @override
  String get onItsWayFromAPaired => 'On its way from a paired device';

  @override
  String get newNote => 'New note';

  @override
  String get noteMarkdown => 'Note (Markdown)';

  @override
  String get everyMeetingIsAlreadyInA =>
      'Every meeting is already in a project.';

  @override
  String get add => 'Add';

  @override
  String get moveToProject => 'Move to project';

  @override
  String get noProject => 'No project';

  @override
  String get newProject => 'New project…';

  @override
  String get newProject2 => 'New project';

  @override
  String get create => 'Create';

  @override
  String get theProjectCouldNotBeCreated => 'The project could not be created.';

  @override
  String get theMeetingCouldNotBeMoved => 'The meeting could not be moved.';

  @override
  String get removedFromItsProject => 'Removed from its project';

  @override
  String movedTo(Object project) {
    return 'Moved to $project';
  }

  @override
  String get searchProjects => 'Search projects';

  @override
  String get noProjectsYetAProjectGathers =>
      'No projects yet. A project gathers meetings with the files, notes and e-mails that belong to them.';

  @override
  String get theRecordingIsStillStartingTry =>
      'The recording is still starting; try again.';

  @override
  String recording(Object title) {
    return 'Recording \"$title\"';
  }

  @override
  String get recording2 => 'Recording';

  @override
  String paused(Object title) {
    return 'Paused \"$title\"';
  }

  @override
  String get paused2 => 'Paused';

  @override
  String get finishingTranscription => 'Finishing transcription…';

  @override
  String get readyToRecord => 'Ready to record';

  @override
  String get meetingTitle => 'Meeting title';

  @override
  String get enterMeetingTitleOptional => 'Enter meeting title (optional)';

  @override
  String get leaveEmptyToNameTheMeeting =>
      'Leave empty to name the meeting by its start time';

  @override
  String get startingRecording => 'Starting recording';

  @override
  String get startRecording => 'Start recording';

  @override
  String get pause => 'Pause';

  @override
  String get resume => 'Resume';

  @override
  String get stop => 'Stop';

  @override
  String get language => 'Language';

  @override
  String get aChangeAppliesToWhatIs => 'A change applies to what is said next';

  @override
  String get liveTranscript => 'Live transcript';

  @override
  String get theTranscriptAppearsHereAFew =>
      'The transcript appears here a few seconds after each pause in speech.';

  @override
  String get nearbyDevicesSeeThisNameWhile =>
      'Nearby devices see this name while this device is discoverable. Devices already paired keep the name they learned.';

  @override
  String get meetingRenamed => 'Meeting renamed';

  @override
  String get thisTitleIsnTAllowed => 'This title isn\'t allowed.';

  @override
  String get couldnTSaveTheNewTitle =>
      'Couldn\'t save the new title. Try again.';

  @override
  String get theNoteTakingEngineIsnT =>
      'The note-taking engine isn\'t available right now.';

  @override
  String get thisMeetingNoLongerExists2 => 'This meeting no longer exists.';

  @override
  String get couldnTRenameTheMeeting => 'Couldn\'t rename the meeting.';

  @override
  String get thisFileIsStillOnIts =>
      'This file is still on its way from a paired device.';

  @override
  String get thisPictureCannotBeShown => 'This picture cannot be shown.';

  @override
  String get thisKindOfDocumentCanT =>
      'This kind of document can\'t be shown in the app yet.';

  @override
  String get thisFileCannotBeOpened => 'This file cannot be opened.';

  @override
  String get thisFileCannotBeRead => 'This file cannot be read.';

  @override
  String get thisEMailCannotBeRead => 'This e-mail cannot be read.';

  @override
  String get noSubject => '(no subject)';

  @override
  String get from => 'From';

  @override
  String get to => 'To';

  @override
  String get cc => 'Cc';

  @override
  String get sent => 'Sent';

  @override
  String attachments(Object attachmentNames) {
    return 'Attachments: $attachmentNames';
  }

  @override
  String get theRestOfThisEMail =>
      'The rest of this e-mail is too long to show.';

  @override
  String get searching => 'Searching…';

  @override
  String get nothingFound => 'Nothing found.';

  @override
  String get search2 => 'Search';

  @override
  String get pairYourDevicesAndSeeWhich =>
      'Pair your devices and see which ones are connected.';

  @override
  String get transcriptionModel => 'Transcription model';

  @override
  String get chooseTheWhisperModel => 'Choose the Whisper model.';

  @override
  String get languageModelsForMeetingSummariesAnd =>
      'Language models for meeting summaries, and the summary prompt.';

  @override
  String get keepProjectsAndMeetingsAsFiles =>
      'Keep projects and meetings as files you can edit with any editor.';

  @override
  String get letYourOwnAssistantSuchAs =>
      'Let your own assistant, such as Claude Code, read your notes.';

  @override
  String get appLock => 'App lock';

  @override
  String get defaultLanguage => 'Default language';

  @override
  String get usedForNewRecordingsAutoDetects =>
      'Used for new recordings. Auto detects the language as people speak.';

  @override
  String get transcribeOn => 'Transcribe on';

  @override
  String get anotherDeviceReceivesTheRecordingS =>
      'Another device receives the recording\'s audio, encrypted, and deletes it once transcribed. If it is unavailable, this device transcribes instead.';

  @override
  String get telemetry => 'Telemetry';

  @override
  String get howLongTranscriptionTakesOnEach =>
      'How long transcription takes on each device, and its estimated energy use.';

  @override
  String get logLevel => 'Log level';

  @override
  String get howMuchDetailTheAppWrites =>
      'How much detail the app writes to its logs. Debug and Trace help diagnose problems but write more.';

  @override
  String get custom => 'Custom';

  @override
  String get shareLogs => 'Share logs';

  @override
  String get theLast7DaysOfDiagnostic =>
      'The last 7 days of diagnostic logs, for example to attach to a bug report.';

  @override
  String get unableToReadTheAppLock => 'Unable to read the app lock setting.';

  @override
  String get needsAndroid10OrLater => 'Needs Android 10 or later.';

  @override
  String get setAScreenLockInAndroid =>
      'Set a screen lock in Android settings to use App lock.';

  @override
  String askForYourFingerprintFaceOr(Object lockAfterBackground) {
    return 'Ask for your fingerprint, face, or screen lock when you return to Koalino after $lockAfterBackground seconds away. Recording and sync keep running while it is locked.';
  }

  @override
  String get otherNetworks => 'Other networks';

  @override
  String get aRelayOnYourOwnAlways =>
      'A relay on your own always-on device lets your devices reach each other away from the same Wi-Fi. Syncing on the same network keeps working without it.';

  @override
  String get relayAddress => 'Relay address';

  @override
  String get deviceAddressesOptionalOnePerLine =>
      'Device addresses (optional, one per line)';

  @override
  String get saveAddresses => 'Save addresses';

  @override
  String get nothingThatWasSharedCouldBe =>
      'Nothing that was shared could be read.';

  @override
  String couldNotBeRead(Object message, Object failedCount) {
    return '$message; $failedCount could not be read';
  }

  @override
  String saveAsTheSummaryOf(Object title) {
    return 'Save as the summary of \"$title\"';
  }

  @override
  String get addToAProjectInstead => 'Add to a project instead';

  @override
  String get splitThisMeeting => 'Split this meeting?';

  @override
  String itWasRecordedOnAndThe(Object devices, Object recordings) {
    return 'It was recorded on $devices, and the app joined the recordings. Splitting lists the other $recordings on its own again, with its own transcript, here and on your paired devices. They are not joined again. This meeting keeps its transcript.';
  }

  @override
  String get unableToSplitThisMeeting => 'Unable to split this meeting.';

  @override
  String get telemetryDashboard => 'Telemetry Dashboard';

  @override
  String get transcription => 'Transcription';

  @override
  String get latency => 'Latency';

  @override
  String get tasks => 'Tasks';

  @override
  String get power => 'Power';

  @override
  String get transcriptionDurationOverTime =>
      'Transcription Duration Over Time';

  @override
  String get latencyDistribution => 'Latency Distribution';

  @override
  String get taskTypeBreakdown => 'Task Type Breakdown';

  @override
  String get powerConsumptionEstimate => 'Power Consumption Estimate';

  @override
  String get fastestLeastAccurate => 'Fastest, least accurate';

  @override
  String get fastManyMistakesTheAppShips =>
      'Fast, many mistakes. The app ships with this one';

  @override
  String get goodBalanceOnMostComputers => 'Good balance on most computers';

  @override
  String get accurateSlowerNeedsAbout2Gb =>
      'Accurate, slower, needs about 2 GB of memory';

  @override
  String get olderNeedsAPowerfulComputer => 'Older, needs a powerful computer';

  @override
  String get mostAccurateNeedsAPowerfulComputer =>
      'Most accurate, needs a powerful computer';

  @override
  String get veryAccurateBestWithAGraphics =>
      'Very accurate. Best with a graphics card';

  @override
  String get thisModelMayBeTooLarge => 'This model may be too large';

  @override
  String mayNeedMoreMemoryThanThis(Object model) {
    return '$model may need more memory than this device has, and recordings could fail or run slowly.';
  }

  @override
  String get downloadAnyway => 'Download anyway';

  @override
  String get transcriptionSettingsAreUnavailable =>
      'Transcription settings are unavailable.';

  @override
  String get downloadFromQualcomm => 'Download from Qualcomm?';

  @override
  String get about360MbIsDownloadedFrom =>
      'About 360 MB is downloaded from Qualcomm and needs about 460 MB of space:\n\n• Qualcomm\'s QNN runtime, which lets apps use the NPU. It is licensed under Qualcomm\'s AI Stack License, not the app\'s licence.\n• Whisper Small, compiled by Qualcomm AI Hub for this phone\'s NPU.\n\nEach file is checked before it is used. Your recordings never leave the phone. By downloading, you accept Qualcomm\'s terms.';

  @override
  String get acceptAndDownload => 'Accept and download';

  @override
  String get downloadingTheNpuFiles => 'Downloading the NPU files…';

  @override
  String get transcribeOnTheNpu => 'Transcribe on the NPU';

  @override
  String get thisPhoneHasAnAiChip =>
      'This phone has an AI chip (NPU) that can run Whisper Small, leaving the processor free: 30 seconds of speech take about 2 seconds. It needs a download from Qualcomm.';

  @override
  String get download => 'Download';

  @override
  String get runsWhisperSmallOnThePhone =>
      'Runs Whisper Small on the phone\'s AI chip, in place of the model chosen below, and for the after-meeting pass unless it has its own model. Word times are approximate.';

  @override
  String get removeTheNpuFiles => 'Remove the NPU files';

  @override
  String get alsoRecordTheComputerSSound => 'Also record the computer\'s sound';

  @override
  String get transcribesWhatTheComputerPlaysSuch =>
      'Transcribes what the computer plays, such as the other side of a call, beside the microphone. When the microphone also hears the speakers, the repeated words are kept once.';

  @override
  String get improveTheTranscriptAfterTheMeeting =>
      'Improve the transcript after the meeting';

  @override
  String get transcribesEachMeetingAgainWithMore =>
      'Transcribes each meeting again with more context once it ends; a phone waits until it is charging. The speech is kept encrypted until then, then deleted.';

  @override
  String get modelForThisPass => 'Model for this pass';

  @override
  String get aLargerModelIsSlowerBut =>
      'A larger model is slower but more accurate, and the pass has time after the meeting. Download models above.';

  @override
  String get sameAsForRecording => 'Same as for recording';

  @override
  String get keepSpeechForCombining => 'Keep speech for combining';

  @override
  String get whenAMeetingWasAlsoRecorded =>
      'When a meeting was also recorded on another device, its speech waits this long to be combined, then is deleted.';

  @override
  String get whisperModel => 'Whisper model';

  @override
  String get biggerModelsMakeFewerMistakesAnd =>
      'Bigger models make fewer mistakes, and need more time and memory. Downloads come from Hugging Face. This choice applies only to this device.';

  @override
  String automatic2(Object fileName, Object mode) {
    return 'Automatic: $fileName$mode';
  }

  @override
  String usesUntilIsDownloadedTheBest(
      Object fileName, Object waiting, Object reason) {
    return 'Uses $fileName until $waiting is downloaded, the best fit for this device. $reason';
  }

  @override
  String picksTheBestModelForThis(Object fileName, Object mode) {
    return 'Picks the best model for this device: $fileName$mode.';
  }

  @override
  String download2(Object sizeBytes) {
    return 'Download $sizeBytes';
  }

  @override
  String tryLabel(Object model) {
    return 'Try $model';
  }

  @override
  String get use => 'Use';

  @override
  String suggestedForThisDevice(Object model, Object mode) {
    return 'Suggested for this device: $model$mode';
  }

  @override
  String get efficientMode => 'Efficient mode';

  @override
  String runsTheSameModelsOnThe(Object gpuName) {
    return 'Runs the same models on the graphics chip ($gpuName), usually faster. Off uses the processor, as standard.';
  }

  @override
  String get theGraphicsChipStoppedTheApp =>
      'The graphics chip stopped the app last time, so this device is transcribing with its processor.';

  @override
  String get tryTheGraphicsChipAgain => 'Try the graphics chip again';

  @override
  String get onThisDevice => 'on this device';

  @override
  String get usedByAutomatic => 'used by Automatic';

  @override
  String get englishOnly => 'English only';

  @override
  String get mayBeTooLargeForThis => 'may be too large for this device';

  @override
  String get doesnTMatchYourLanguage => 'doesn\'t match your language';

  @override
  String get notEnoughFreeSpace => 'not enough free space';

  @override
  String get cancelDownload => 'Cancel download';

  @override
  String get glossary => 'Glossary';

  @override
  String get namesProjectNamesAndTermsOne =>
      'Names, project names and terms, one per line. Transcription spells them as written here.';

  @override
  String get saveGlossary => 'Save glossary';

  @override
  String get voiceGallery => 'Voice Gallery';

  @override
  String get tryAgain => 'Try again';

  @override
  String get unknownVoices => 'Unknown voices';

  @override
  String get noUnknownSpeakersToReview => 'No unknown speakers to review';

  @override
  String get savedPeople => 'Saved people';

  @override
  String get noSavedPeopleYetNameA =>
      'No saved people yet. Name a voice to add someone.';

  @override
  String delete4(Object displayName) {
    return 'Delete $displayName';
  }

  @override
  String get playing => 'Playing…';

  @override
  String get untitledMeeting => 'Untitled meeting';

  @override
  String get play => 'Play';

  @override
  String get deleteThisVoice => 'Delete this voice';

  @override
  String theCallShowedSpeakingCheckThen(Object suggestedName) {
    return 'The call showed $suggestedName speaking. Check, then Assign.';
  }

  @override
  String get chooseASavedPerson => 'Choose a saved person';

  @override
  String get assign => 'Assign';

  @override
  String get deleteTheseVoices => 'Delete these voices';

  @override
  String get theseSoundLikeOnePersonUntick =>
      'These sound like one person. Untick any that is someone else.';

  @override
  String itsVoiceSamplesGoForGood(Object voices) {
    return 'Its $voices voice samples go for good, here and on your paired devices. The transcripts stay.';
  }

  @override
  String get itsVoiceSampleGoesForGood =>
      'Its voice sample goes for good, here and on your paired devices. The transcript stays.';

  @override
  String get theirNameAndVoiceSamplesGo =>
      'Their name and voice samples go for good, here and on your paired devices. Transcripts stay and number them as a speaker.';

  @override
  String unknownSpeaker(Object index) {
    return 'Unknown speaker $index';
  }

  @override
  String attachedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Attached $count files',
      one: 'Attached 1 file',
    );
    return '$_temp0';
  }

  @override
  String addedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Added $count files',
      one: 'Added 1 file',
    );
    return '$_temp0';
  }

  @override
  String emailAttachmentsLeft(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count e-mail attachments were left in the e-mail',
      one: '1 e-mail attachment was left in the e-mail',
    );
    return '$_temp0';
  }

  @override
  String addedMeetings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Added $count meetings',
      one: 'Added 1 meeting',
    );
    return '$_temp0';
  }

  @override
  String otherRecordings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count recordings',
      one: 'recording',
    );
    return '$_temp0';
  }

  @override
  String get severalDevices => 'several devices';

  @override
  String meetingCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count meetings',
      one: '1 meeting',
    );
    return '$_temp0';
  }

  @override
  String fileCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count files',
      one: '1 file',
    );
    return '$_temp0';
  }

  @override
  String chunkCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count chunks',
      one: '1 chunk',
    );
    return '$_temp0';
  }

  @override
  String resultCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count results',
      one: '1 result',
    );
    return '$_temp0';
  }

  @override
  String get moreNotShown => ', more not shown: add words to narrow it';

  @override
  String cpuCores(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count CPU cores',
      one: '1 CPU core',
    );
    return '$_temp0';
  }

  @override
  String dayCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count days',
      one: '1 day',
    );
    return '$_temp0';
  }

  @override
  String voiceSampleCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count voice samples',
      one: '1 voice sample',
    );
    return '$_temp0';
  }

  @override
  String picturesIncluded(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count pictures were included',
      one: '1 picture was included',
    );
    return '$_temp0';
  }

  @override
  String partCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count parts',
      one: '1 part',
    );
    return '$_temp0';
  }

  @override
  String inMeetings(Object parts, Object meetings) {
    return '$parts in $meetings';
  }

  @override
  String assignedVoicesTo(int count, Object name) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Assigned $count voices to $name',
      one: 'Assigned to $name',
    );
    return '$_temp0';
  }

  @override
  String addSharedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Add $count shared files to a project',
      one: 'Add the shared file to a project',
    );
    return '$_temp0';
  }

  @override
  String keepSpeechSaved(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Saved. Speech now waits up to $count days to be combined.',
      one: 'Saved. Speech now waits up to 1 day to be combined.',
    );
    return '$_temp0';
  }

  @override
  String titlesCanBeAtMost(Object max) {
    return 'Titles can be at most $max characters.';
  }

  @override
  String promptUnknownPlaceholder(Object example) {
    return 'The prompt has an unknown or unclosed $example.';
  }

  @override
  String get live => '● Live';

  @override
  String get offline => '○ Offline';

  @override
  String minutesShort(Object minutes) {
    return '$minutes min';
  }

  @override
  String get appLanguage => 'App language';

  @override
  String get appLanguageDescription =>
      'The language of Koalino\'s menus and messages.';

  @override
  String get systemLanguage => 'System default';

  @override
  String photosAddedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count photos added',
      one: '1 photo added',
    );
    return '$_temp0';
  }

  @override
  String moreModels(int count) {
    return 'More models ($count)';
  }

  @override
  String get moreModelsHint =>
      'Smaller versions that make a few more mistakes, English-only versions, and models that may not fit this device.';

  @override
  String get microphoneSilencedByCall =>
      'Android is silencing the microphone while another app uses it for a call: nothing is recorded until the call ends.';
}
