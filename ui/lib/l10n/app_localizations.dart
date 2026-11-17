import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:flutter_localizations/flutter_localizations.dart';
import 'package:intl/intl.dart' as intl;

import 'app_localizations_ar.dart';
import 'app_localizations_de.dart';
import 'app_localizations_en.dart';
import 'app_localizations_es.dart';
import 'app_localizations_fr.dart';
import 'app_localizations_it.dart';
import 'app_localizations_ja.dart';
import 'app_localizations_pt.dart';
import 'app_localizations_ru.dart';
import 'app_localizations_zh.dart';

// ignore_for_file: type=lint

/// Callers can lookup localized strings with an instance of AppLocalizations
/// returned by `AppLocalizations.of(context)`.
///
/// Applications need to include `AppLocalizations.delegate()` in their app's
/// `localizationDelegates` list, and the locales they support in the app's
/// `supportedLocales` list. For example:
///
/// ```dart
/// import 'l10n/app_localizations.dart';
///
/// return MaterialApp(
///   localizationsDelegates: AppLocalizations.localizationsDelegates,
///   supportedLocales: AppLocalizations.supportedLocales,
///   home: MyApplicationHome(),
/// );
/// ```
///
/// ## Update pubspec.yaml
///
/// Please make sure to update your pubspec.yaml to include the following
/// packages:
///
/// ```yaml
/// dependencies:
///   # Internationalization support.
///   flutter_localizations:
///     sdk: flutter
///   intl: any # Use the pinned version from flutter_localizations
///
///   # Rest of dependencies
/// ```
///
/// ## iOS Applications
///
/// iOS applications define key application metadata, including supported
/// locales, in an Info.plist file that is built into the application bundle.
/// To configure the locales supported by your app, you’ll need to edit this
/// file.
///
/// First, open your project’s ios/Runner.xcworkspace Xcode workspace file.
/// Then, in the Project Navigator, open the Info.plist file under the Runner
/// project’s Runner folder.
///
/// Next, select the Information Property List item, select Add Item from the
/// Editor menu, then select Localizations from the pop-up menu.
///
/// Select and expand the newly-created Localizations item then, for each
/// locale your application supports, add a new item and select the locale
/// you wish to add from the pop-up menu in the Value field. This list should
/// be consistent with the languages listed in the AppLocalizations.supportedLocales
/// property.
abstract class AppLocalizations {
  AppLocalizations(String locale)
      : localeName = intl.Intl.canonicalizedLocale(locale.toString());

  final String localeName;

  static AppLocalizations of(BuildContext context) {
    return Localizations.of<AppLocalizations>(context, AppLocalizations)!;
  }

  static const LocalizationsDelegate<AppLocalizations> delegate =
      _AppLocalizationsDelegate();

  /// A list of this localizations delegate along with the default localizations
  /// delegates.
  ///
  /// Returns a list of localizations delegates containing this delegate along with
  /// GlobalMaterialLocalizations.delegate, GlobalCupertinoLocalizations.delegate,
  /// and GlobalWidgetsLocalizations.delegate.
  ///
  /// Additional delegates can be added by appending to this list in
  /// MaterialApp. This list does not have to be used at all if a custom list
  /// of delegates is preferred or required.
  static const List<LocalizationsDelegate<dynamic>> localizationsDelegates =
      <LocalizationsDelegate<dynamic>>[
    delegate,
    GlobalMaterialLocalizations.delegate,
    GlobalCupertinoLocalizations.delegate,
    GlobalWidgetsLocalizations.delegate,
  ];

  /// A list of this localizations delegate's supported locales.
  static const List<Locale> supportedLocales = <Locale>[
    Locale('ar'),
    Locale('de'),
    Locale('en'),
    Locale('es'),
    Locale('fr'),
    Locale('it'),
    Locale('ja'),
    Locale('pt'),
    Locale('ru'),
    Locale('zh')
  ];

  /// No description provided for @unableToCheckTheAppLock.
  ///
  /// In en, this message translates to:
  /// **'Unable to check the app lock. Try again.'**
  String get unableToCheckTheAppLock;

  /// No description provided for @unableToShowTheUnlockPrompt.
  ///
  /// In en, this message translates to:
  /// **'Unable to show the unlock prompt. Try again.'**
  String get unableToShowTheUnlockPrompt;

  /// No description provided for @setAScreenLockOnThis.
  ///
  /// In en, this message translates to:
  /// **'Set a screen lock on this device first.'**
  String get setAScreenLockOnThis;

  /// No description provided for @unableToChangeTheAppLock.
  ///
  /// In en, this message translates to:
  /// **'Unable to change the app lock. Try again.'**
  String get unableToChangeTheAppLock;

  /// No description provided for @aCallApp.
  ///
  /// In en, this message translates to:
  /// **'A call app'**
  String get aCallApp;

  /// No description provided for @call.
  ///
  /// In en, this message translates to:
  /// **'Call'**
  String get call;

  /// No description provided for @call2.
  ///
  /// In en, this message translates to:
  /// **'{app} call'**
  String call2(Object app);

  /// No description provided for @theCallEndedSoTheRecording.
  ///
  /// In en, this message translates to:
  /// **'The {tiedApp} call ended, so the recording stopped.'**
  String theCallEndedSoTheRecording(Object tiedApp);

  /// No description provided for @restartYourBrowserAndCallApps.
  ///
  /// In en, this message translates to:
  /// **'Restart your browser and call apps once, so Koalino can read who is speaking in calls.'**
  String get restartYourBrowserAndCallApps;

  /// No description provided for @unableToLoadDevices.
  ///
  /// In en, this message translates to:
  /// **'Unable to load devices'**
  String get unableToLoadDevices;

  /// No description provided for @thisDeviceIsNotOnA.
  ///
  /// In en, this message translates to:
  /// **'This device is not on a network yet. Connect it and try again.'**
  String get thisDeviceIsNotOnA;

  /// No description provided for @thatDidNotWorkTryAgain.
  ///
  /// In en, this message translates to:
  /// **'That did not work. Try again.'**
  String get thatDidNotWorkTryAgain;

  /// No description provided for @use1To64CharactersWithout.
  ///
  /// In en, this message translates to:
  /// **'Use 1 to 64 characters, without line breaks.'**
  String get use1To64CharactersWithout;

  /// No description provided for @theNameCouldNotBeSaved.
  ///
  /// In en, this message translates to:
  /// **'The name could not be saved. Try again.'**
  String get theNameCouldNotBeSaved;

  /// No description provided for @thisInviteHasExpiredMakeA.
  ///
  /// In en, this message translates to:
  /// **'This invite has expired. Make a new one on the other device.'**
  String get thisInviteHasExpiredMakeA;

  /// No description provided for @thisInviteWasMadeOnThis.
  ///
  /// In en, this message translates to:
  /// **'This invite was made on this device. Use it on the other one.'**
  String get thisInviteWasMadeOnThis;

  /// No description provided for @thatDeviceIsAlreadyPaired.
  ///
  /// In en, this message translates to:
  /// **'That device is already paired.'**
  String get thatDeviceIsAlreadyPaired;

  /// No description provided for @thatIsNotAValidInvite.
  ///
  /// In en, this message translates to:
  /// **'That is not a valid invite. Copy it again.'**
  String get thatIsNotAValidInvite;

  /// No description provided for @deviceSyncIsNotRunning.
  ///
  /// In en, this message translates to:
  /// **'Device sync is not running.'**
  String get deviceSyncIsNotRunning;

  /// No description provided for @theOtherDeviceIsNoLonger.
  ///
  /// In en, this message translates to:
  /// **'The other device is no longer pairing. Try again.'**
  String get theOtherDeviceIsNoLonger;

  /// No description provided for @theOtherAppCouldNotBe.
  ///
  /// In en, this message translates to:
  /// **'The other app could not be opened.'**
  String get theOtherAppCouldNotBe;

  /// No description provided for @theOtherApp.
  ///
  /// In en, this message translates to:
  /// **'the other app'**
  String get theOtherApp;

  /// No description provided for @app.
  ///
  /// In en, this message translates to:
  /// **'{displayName} app'**
  String app(Object displayName);

  /// No description provided for @unableToChangeTheLogLevel.
  ///
  /// In en, this message translates to:
  /// **'Unable to change the log level'**
  String get unableToChangeTheLogLevel;

  /// No description provided for @logsExportedTo.
  ///
  /// In en, this message translates to:
  /// **'Logs exported to {filePath}'**
  String logsExportedTo(Object filePath);

  /// No description provided for @unableToShareTheLogs.
  ///
  /// In en, this message translates to:
  /// **'Unable to share the logs'**
  String get unableToShareTheLogs;

  /// No description provided for @noLogsToExportYet.
  ///
  /// In en, this message translates to:
  /// **'No logs to export yet'**
  String get noLogsToExportYet;

  /// No description provided for @unableToExportTheLogs.
  ///
  /// In en, this message translates to:
  /// **'Unable to export the logs'**
  String get unableToExportTheLogs;

  /// No description provided for @couldNotBeAttached.
  ///
  /// In en, this message translates to:
  /// **'{attached}; {failedPaths} could not be attached'**
  String couldNotBeAttached(Object attached, Object failedPaths);

  /// No description provided for @theFilesCouldNotBeAttached.
  ///
  /// In en, this message translates to:
  /// **'The files could not be attached.'**
  String get theFilesCouldNotBeAttached;

  /// No description provided for @unableToShareTheExport.
  ///
  /// In en, this message translates to:
  /// **'Unable to share the export'**
  String get unableToShareTheExport;

  /// No description provided for @exportedTo.
  ///
  /// In en, this message translates to:
  /// **'Exported to {filePath}'**
  String exportedTo(Object filePath);

  /// No description provided for @exportSaved.
  ///
  /// In en, this message translates to:
  /// **'Export saved'**
  String get exportSaved;

  /// No description provided for @unableToSaveTheExport.
  ///
  /// In en, this message translates to:
  /// **'Unable to save the export'**
  String get unableToSaveTheExport;

  /// No description provided for @thisMeetingNoLongerExists.
  ///
  /// In en, this message translates to:
  /// **'This meeting no longer exists'**
  String get thisMeetingNoLongerExists;

  /// No description provided for @exportIsUnavailableUntilTheApp.
  ///
  /// In en, this message translates to:
  /// **'Export is unavailable until the app has started'**
  String get exportIsUnavailableUntilTheApp;

  /// No description provided for @unableToExportTheMeeting.
  ///
  /// In en, this message translates to:
  /// **'Unable to export the meeting'**
  String get unableToExportTheMeeting;

  /// No description provided for @unableToLoadMeetings.
  ///
  /// In en, this message translates to:
  /// **'Unable to load meetings'**
  String get unableToLoadMeetings;

  /// No description provided for @unableToLoadThisMeeting.
  ///
  /// In en, this message translates to:
  /// **'Unable to load this meeting'**
  String get unableToLoadThisMeeting;

  /// No description provided for @enterATitle.
  ///
  /// In en, this message translates to:
  /// **'Enter a title.'**
  String get enterATitle;

  /// No description provided for @titlesCanBeAtMostCharacters.
  ///
  /// In en, this message translates to:
  /// **'Titles can be at most {maxMeetingTitleCharacters} characters (you have {runes}).'**
  String titlesCanBeAtMostCharacters(
      Object maxMeetingTitleCharacters, Object runes);

  /// No description provided for @titlesCanTContainLineBreaks.
  ///
  /// In en, this message translates to:
  /// **'Titles can\'t contain line breaks or control characters.'**
  String get titlesCanTContainLineBreaks;

  /// No description provided for @notPairedYetMakeTheRelay.
  ///
  /// In en, this message translates to:
  /// **'Not paired yet. Make the relay discoverable, then pair with it under Nearby devices in Devices.'**
  String get notPairedYetMakeTheRelay;

  /// No description provided for @connectingToTheRelay.
  ///
  /// In en, this message translates to:
  /// **'Connecting to the relay…'**
  String get connectingToTheRelay;

  /// No description provided for @connectedToTheRelayAskingIt.
  ///
  /// In en, this message translates to:
  /// **'Connected to the relay. Asking it to accept connections…'**
  String get connectedToTheRelayAskingIt;

  /// No description provided for @connectedYourOtherDevicesCanReach.
  ///
  /// In en, this message translates to:
  /// **'Connected. Your other devices can reach this one through the relay.'**
  String get connectedYourOtherDevicesCanReach;

  /// No description provided for @savedThisAppliesNowWithNo.
  ///
  /// In en, this message translates to:
  /// **'Saved. This applies now, with no restart.'**
  String get savedThisAppliesNowWithNo;

  /// No description provided for @thatRelayAddressIsNotValid.
  ///
  /// In en, this message translates to:
  /// **'That relay address is not valid. It looks like /ip4/203.0.113.5/tcp/4001/p2p/12D3Koo…'**
  String get thatRelayAddressIsNotValid;

  /// No description provided for @aDeviceAddressIsNotValid.
  ///
  /// In en, this message translates to:
  /// **'A device address is not valid. Each ends in /p2p/ and the device\'s ID.'**
  String get aDeviceAddressIsNotValid;

  /// No description provided for @unableToSaveTheAddresses.
  ///
  /// In en, this message translates to:
  /// **'Unable to save the addresses.'**
  String get unableToSaveTheAddresses;

  /// No description provided for @thePhotoCouldNotBeSaved.
  ///
  /// In en, this message translates to:
  /// **'The photo could not be saved.'**
  String get thePhotoCouldNotBeSaved;

  /// No description provided for @enterAName.
  ///
  /// In en, this message translates to:
  /// **'Enter a name.'**
  String get enterAName;

  /// No description provided for @namesCanBeAtMostCharacters.
  ///
  /// In en, this message translates to:
  /// **'Names can be at most {maxMeetingTitleCharacters} characters.'**
  String namesCanBeAtMostCharacters(Object maxMeetingTitleCharacters);

  /// No description provided for @namesCanTContainLineBreaks.
  ///
  /// In en, this message translates to:
  /// **'Names can\'t contain line breaks or control characters.'**
  String get namesCanTContainLineBreaks;

  /// No description provided for @unableToLoadProjects.
  ///
  /// In en, this message translates to:
  /// **'Unable to load projects'**
  String get unableToLoadProjects;

  /// No description provided for @unableToLoadThisProject.
  ///
  /// In en, this message translates to:
  /// **'Unable to load this project'**
  String get unableToLoadThisProject;

  /// No description provided for @theFilesCouldNotBeAdded.
  ///
  /// In en, this message translates to:
  /// **'The files could not be added.'**
  String get theFilesCouldNotBeAdded;

  /// No description provided for @couldNotBeAdded.
  ///
  /// In en, this message translates to:
  /// **'{failedPaths} could not be added'**
  String couldNotBeAdded(Object failedPaths);

  /// No description provided for @unableToStartRecording.
  ///
  /// In en, this message translates to:
  /// **'Unable to start recording'**
  String get unableToStartRecording;

  /// No description provided for @unableToPauseRecording.
  ///
  /// In en, this message translates to:
  /// **'Unable to pause recording'**
  String get unableToPauseRecording;

  /// No description provided for @unableToResumeRecording.
  ///
  /// In en, this message translates to:
  /// **'Unable to resume recording'**
  String get unableToResumeRecording;

  /// No description provided for @unableToStopRecording.
  ///
  /// In en, this message translates to:
  /// **'Unable to stop recording'**
  String get unableToStopRecording;

  /// No description provided for @recordingSaved.
  ///
  /// In en, this message translates to:
  /// **'Recording saved'**
  String get recordingSaved;

  /// No description provided for @microphoneAccessIsNeededToRecord.
  ///
  /// In en, this message translates to:
  /// **'Microphone access is needed to record'**
  String get microphoneAccessIsNeededToRecord;

  /// No description provided for @keepTheAppOpenAndroidDid.
  ///
  /// In en, this message translates to:
  /// **'Keep the app open: Android did not allow recording in the background'**
  String get keepTheAppOpenAndroidDid;

  /// No description provided for @unableToOpenTheMicrophone.
  ///
  /// In en, this message translates to:
  /// **'Unable to open the microphone'**
  String get unableToOpenTheMicrophone;

  /// No description provided for @saved.
  ///
  /// In en, this message translates to:
  /// **'Saved {label}.'**
  String saved(Object label);

  /// No description provided for @removed.
  ///
  /// In en, this message translates to:
  /// **'Removed {label}.'**
  String removed(Object label);

  /// No description provided for @isNowTheDefault.
  ///
  /// In en, this message translates to:
  /// **'{id} is now the default.'**
  String isNowTheDefault(Object id);

  /// No description provided for @answered.
  ///
  /// In en, this message translates to:
  /// **'{id} answered: {reply}'**
  String answered(Object id, Object reply);

  /// No description provided for @promptSaved.
  ///
  /// In en, this message translates to:
  /// **'Prompt saved.'**
  String get promptSaved;

  /// No description provided for @theBuiltInPromptIsBack.
  ///
  /// In en, this message translates to:
  /// **'The built-in prompt is back.'**
  String get theBuiltInPromptIsBack;

  /// No description provided for @finishedMeetingsWillBeSummarizedAutomatically.
  ///
  /// In en, this message translates to:
  /// **'Finished meetings will be summarized automatically.'**
  String get finishedMeetingsWillBeSummarizedAutomatically;

  /// No description provided for @automaticSummariesAreOff.
  ///
  /// In en, this message translates to:
  /// **'Automatic summaries are off.'**
  String get automaticSummariesAreOff;

  /// No description provided for @thisComputerNoLongerWritesSummaries.
  ///
  /// In en, this message translates to:
  /// **'This computer no longer writes summaries or answers chats for your other devices.'**
  String get thisComputerNoLongerWritesSummaries;

  /// No description provided for @nowWritesTheSummariesAndAnswers.
  ///
  /// In en, this message translates to:
  /// **'{id} now writes the summaries and answers the chats your other devices ask for.'**
  String nowWritesTheSummariesAndAnswers(Object id);

  /// No description provided for @requestCancelled.
  ///
  /// In en, this message translates to:
  /// **'Request cancelled.'**
  String get requestCancelled;

  /// No description provided for @thisMeetingUsesTheSettingsPrompt.
  ///
  /// In en, this message translates to:
  /// **'This meeting uses the Settings prompt again.'**
  String get thisMeetingUsesTheSettingsPrompt;

  /// No description provided for @promptSavedForThisMeeting.
  ///
  /// In en, this message translates to:
  /// **'Prompt saved for this meeting.'**
  String get promptSavedForThisMeeting;

  /// No description provided for @summarySaved.
  ///
  /// In en, this message translates to:
  /// **'Summary saved.'**
  String get summarySaved;

  /// No description provided for @theConnection.
  ///
  /// In en, this message translates to:
  /// **'The connection'**
  String get theConnection;

  /// No description provided for @summaryWritten.
  ///
  /// In en, this message translates to:
  /// **'Summary written.'**
  String get summaryWritten;

  /// No description provided for @theTranscriptWasReadInParts.
  ///
  /// In en, this message translates to:
  /// **' The transcript was read in {parts} parts.'**
  String theTranscriptWasReadInParts(Object parts);

  /// No description provided for @leftOut.
  ///
  /// In en, this message translates to:
  /// **' {sent}, {imagesSkipped} left out.'**
  String leftOut(Object sent, Object imagesSkipped);

  /// No description provided for @theModelStoppedBeforeTheEnd.
  ///
  /// In en, this message translates to:
  /// **' The model stopped before the end.'**
  String get theModelStoppedBeforeTheEnd;

  /// No description provided for @waitingForYourComputerItWrites.
  ///
  /// In en, this message translates to:
  /// **'Waiting for your computer. It writes the summary once it is on, connected to this device, and set to answer in Settings → AI connections.'**
  String get waitingForYourComputerItWrites;

  /// No description provided for @yourComputerIsWritingTheSummary.
  ///
  /// In en, this message translates to:
  /// **'Your computer is writing the summary…'**
  String get yourComputerIsWritingTheSummary;

  /// No description provided for @yourComputerCouldNotWriteThe.
  ///
  /// In en, this message translates to:
  /// **'Your computer could not write the summary: {message}'**
  String yourComputerCouldNotWriteThe(Object message);

  /// No description provided for @unableToRefreshTelemetry.
  ///
  /// In en, this message translates to:
  /// **'Unable to refresh telemetry'**
  String get unableToRefreshTelemetry;

  /// No description provided for @automatic.
  ///
  /// In en, this message translates to:
  /// **'Automatic'**
  String get automatic;

  /// No description provided for @transcriptionLanguage.
  ///
  /// In en, this message translates to:
  /// **'Transcription language'**
  String get transcriptionLanguage;

  /// No description provided for @transcriptionLanguages.
  ///
  /// In en, this message translates to:
  /// **'Transcription languages'**
  String get transcriptionLanguages;

  /// No description provided for @theLanguageCouldNotBeChanged.
  ///
  /// In en, this message translates to:
  /// **'The language could not be changed.'**
  String get theLanguageCouldNotBeChanged;

  /// No description provided for @thisDevice.
  ///
  /// In en, this message translates to:
  /// **'This device'**
  String get thisDevice;

  /// No description provided for @bestAvailable.
  ///
  /// In en, this message translates to:
  /// **'Best available'**
  String get bestAvailable;

  /// No description provided for @aRemovedDevice.
  ///
  /// In en, this message translates to:
  /// **'A removed device'**
  String get aRemovedDevice;

  /// No description provided for @theTranscriptionDeviceCouldNotBe.
  ///
  /// In en, this message translates to:
  /// **'The transcription device could not be changed. Check that it is still paired.'**
  String get theTranscriptionDeviceCouldNotBe;

  /// No description provided for @downloadFinishedYouCanNowChoose.
  ///
  /// In en, this message translates to:
  /// **'Download finished. You can now choose this model.'**
  String get downloadFinishedYouCanNowChoose;

  /// No description provided for @downloadCancelled.
  ///
  /// In en, this message translates to:
  /// **'Download cancelled.'**
  String get downloadCancelled;

  /// No description provided for @theDownloadStoppedNoConnectionTo.
  ///
  /// In en, this message translates to:
  /// **'The download stopped: no connection to the server. Your current model still works.'**
  String get theDownloadStoppedNoConnectionTo;

  /// No description provided for @theDownloadedFileFailedItsCheck.
  ///
  /// In en, this message translates to:
  /// **'The downloaded file failed its check and was deleted. Your current model still works.'**
  String get theDownloadedFileFailedItsCheck;

  /// No description provided for @theModelCouldNotBeSaved.
  ///
  /// In en, this message translates to:
  /// **'The model could not be saved. Check the free space. Your current model still works.'**
  String get theModelCouldNotBeSaved;

  /// No description provided for @theDownloadServerRefusedTheFile.
  ///
  /// In en, this message translates to:
  /// **'The download server refused the file. Your current model still works.'**
  String get theDownloadServerRefusedTheFile;

  /// No description provided for @savedTheNextRecordingUsesThis.
  ///
  /// In en, this message translates to:
  /// **'Saved. The next recording uses this model.'**
  String get savedTheNextRecordingUsesThis;

  /// No description provided for @savedTheNextRecordingUsesThis2.
  ///
  /// In en, this message translates to:
  /// **'Saved. The next recording uses this.'**
  String get savedTheNextRecordingUsesThis2;

  /// No description provided for @savedTheNextRecordingUsesThe.
  ///
  /// In en, this message translates to:
  /// **'Saved. The next recording uses the automatic choice.'**
  String get savedTheNextRecordingUsesThe;

  /// No description provided for @savedMeetingsRecordedFromNowOn.
  ///
  /// In en, this message translates to:
  /// **'Saved. Meetings recorded from now on use this.'**
  String get savedMeetingsRecordedFromNowOn;

  /// No description provided for @savedTheNextRecordingUsesThese.
  ///
  /// In en, this message translates to:
  /// **'Saved. The next recording uses these names and terms.'**
  String get savedTheNextRecordingUsesThese;

  /// No description provided for @eachTermCanHaveAtMost.
  ///
  /// In en, this message translates to:
  /// **'Each term can have at most 60 characters, and all of them together at most 500.'**
  String get eachTermCanHaveAtMost;

  /// No description provided for @chooseAWindowBetween1And.
  ///
  /// In en, this message translates to:
  /// **'Choose a window between 1 and 60 days.'**
  String get chooseAWindowBetween1And;

  /// No description provided for @theDownloadCouldNotStartPlease.
  ///
  /// In en, this message translates to:
  /// **'The download could not start. Please try again.'**
  String get theDownloadCouldNotStartPlease;

  /// No description provided for @savedTheNextRecordingIsTranscribed.
  ///
  /// In en, this message translates to:
  /// **'Saved. The next recording is transcribed on the NPU.'**
  String get savedTheNextRecordingIsTranscribed;

  /// No description provided for @savedTheNextRecordingIsTranscribed2.
  ///
  /// In en, this message translates to:
  /// **'Saved. The next recording is transcribed on the processor.'**
  String get savedTheNextRecordingIsTranscribed2;

  /// No description provided for @theNpuFilesAreNotAll.
  ///
  /// In en, this message translates to:
  /// **'The NPU files are not all downloaded yet.'**
  String get theNpuFilesAreNotAll;

  /// No description provided for @theNpuFilesWereRemoved.
  ///
  /// In en, this message translates to:
  /// **'The NPU files were removed.'**
  String get theNpuFilesWereRemoved;

  /// No description provided for @theNpuFilesCouldNotBe.
  ///
  /// In en, this message translates to:
  /// **'The NPU files could not be removed. Please try again.'**
  String get theNpuFilesCouldNotBe;

  /// No description provided for @downloadedRecordingsNowTranscribeOnThe.
  ///
  /// In en, this message translates to:
  /// **'Downloaded. Recordings now transcribe on the NPU.'**
  String get downloadedRecordingsNowTranscribeOnThe;

  /// No description provided for @theDownloadStoppedNoConnectionTo2.
  ///
  /// In en, this message translates to:
  /// **'The download stopped: no connection to the server.'**
  String get theDownloadStoppedNoConnectionTo2;

  /// No description provided for @aDownloadedFileFailedItsCheck.
  ///
  /// In en, this message translates to:
  /// **'A downloaded file failed its check and was deleted.'**
  String get aDownloadedFileFailedItsCheck;

  /// No description provided for @theFilesCouldNotBeSaved.
  ///
  /// In en, this message translates to:
  /// **'The files could not be saved. Check the free space (about 800 MB is needed).'**
  String get theFilesCouldNotBeSaved;

  /// No description provided for @theDownloadServerRefusedTheFiles.
  ///
  /// In en, this message translates to:
  /// **'The download server refused the files.'**
  String get theDownloadServerRefusedTheFiles;

  /// No description provided for @theNextRecordingTriesTheGraphics.
  ///
  /// In en, this message translates to:
  /// **'The next recording tries the graphics chip again.'**
  String get theNextRecordingTriesTheGraphics;

  /// No description provided for @thatModelWonTBeProposed.
  ///
  /// In en, this message translates to:
  /// **'That model won\'t be proposed again.'**
  String get thatModelWonTBeProposed;

  /// No description provided for @modelRemoved.
  ///
  /// In en, this message translates to:
  /// **'Model removed.'**
  String get modelRemoved;

  /// No description provided for @anotherModelIsDownloadingWaitFor.
  ///
  /// In en, this message translates to:
  /// **'Another model is downloading. Wait for it to finish.'**
  String get anotherModelIsDownloadingWaitFor;

  /// No description provided for @thereIsnTEnoughFreeSpace.
  ///
  /// In en, this message translates to:
  /// **'There isn\'t enough free space for this model.'**
  String get thereIsnTEnoughFreeSpace;

  /// No description provided for @thatDidnTWorkPleaseTry.
  ///
  /// In en, this message translates to:
  /// **'That didn\'t work. Please try again.'**
  String get thatDidnTWorkPleaseTry;

  /// No description provided for @unableToLoadVoiceGallery.
  ///
  /// In en, this message translates to:
  /// **'Unable to load Voice Gallery'**
  String get unableToLoadVoiceGallery;

  /// No description provided for @unableToPlayThisVoice.
  ///
  /// In en, this message translates to:
  /// **'Unable to play this voice'**
  String get unableToPlayThisVoice;

  /// No description provided for @unableToDeleteThisVoice.
  ///
  /// In en, this message translates to:
  /// **'Unable to delete this voice'**
  String get unableToDeleteThisVoice;

  /// No description provided for @voiceDeleted.
  ///
  /// In en, this message translates to:
  /// **'Voice deleted'**
  String get voiceDeleted;

  /// No description provided for @unableToDeleteThisPerson.
  ///
  /// In en, this message translates to:
  /// **'Unable to delete this person'**
  String get unableToDeleteThisPerson;

  /// No description provided for @deleted.
  ///
  /// In en, this message translates to:
  /// **'{displayName} deleted'**
  String deleted(Object displayName);

  /// No description provided for @enterANameBeforeAssigning.
  ///
  /// In en, this message translates to:
  /// **'Enter a name before assigning'**
  String get enterANameBeforeAssigning;

  /// No description provided for @assignedTo.
  ///
  /// In en, this message translates to:
  /// **'Assigned to {identityName}'**
  String assignedTo(Object identityName);

  /// No description provided for @assignedToThereWasNotEnough.
  ///
  /// In en, this message translates to:
  /// **'Assigned to {identityName}. There was not enough clear speech to recognize this voice in future recordings.'**
  String assignedToThereWasNotEnough(Object identityName);

  /// No description provided for @unableToAssignTheSpeakerCluster.
  ///
  /// In en, this message translates to:
  /// **'Unable to assign the speaker cluster'**
  String get unableToAssignTheSpeakerCluster;

  /// No description provided for @chooseAtLeastOneVoiceTo.
  ///
  /// In en, this message translates to:
  /// **'Choose at least one voice to name'**
  String get chooseAtLeastOneVoiceTo;

  /// No description provided for @thereWasNotEnoughClearSpeech.
  ///
  /// In en, this message translates to:
  /// **'{assigned}. There was not enough clear speech to recognize this voice in future recordings.'**
  String thereWasNotEnoughClearSpeech(Object assigned);

  /// No description provided for @theSettingCouldNotBeSaved.
  ///
  /// In en, this message translates to:
  /// **'The setting could not be saved.'**
  String get theSettingCouldNotBeSaved;

  /// No description provided for @theAppIsStillStartingTry.
  ///
  /// In en, this message translates to:
  /// **'The app is still starting. Try again.'**
  String get theAppIsStillStartingTry;

  /// No description provided for @somethingWentWrongTryAgain.
  ///
  /// In en, this message translates to:
  /// **'Something went wrong. Try again.'**
  String get somethingWentWrongTryAgain;

  /// No description provided for @isUsingTheMicrophone.
  ///
  /// In en, this message translates to:
  /// **'{app} is using the microphone'**
  String isUsingTheMicrophone(Object app);

  /// No description provided for @recordThisCall.
  ///
  /// In en, this message translates to:
  /// **'Record this call?'**
  String get recordThisCall;

  /// No description provided for @theAiServerRejectedTheRequest.
  ///
  /// In en, this message translates to:
  /// **'The AI server rejected the request.\nThe server said: {detail}'**
  String theAiServerRejectedTheRequest(Object detail);

  /// No description provided for @claudeCodeOrCodexCouldNot.
  ///
  /// In en, this message translates to:
  /// **'Claude Code or Codex could not answer.\nIt said: {detail}'**
  String claudeCodeOrCodexCouldNot(Object detail);

  /// No description provided for @typeAQuestion.
  ///
  /// In en, this message translates to:
  /// **'Type a question.'**
  String get typeAQuestion;

  /// No description provided for @theQuestionIsTooLong.
  ///
  /// In en, this message translates to:
  /// **'The question is too long.'**
  String get theQuestionIsTooLong;

  /// No description provided for @addAnAiConnectionInSettings.
  ///
  /// In en, this message translates to:
  /// **'Add an AI connection in Settings, or choose a default one.'**
  String get addAnAiConnectionInSettings;

  /// No description provided for @thatAiConnectionNoLongerExists.
  ///
  /// In en, this message translates to:
  /// **'That AI connection no longer exists.'**
  String get thatAiConnectionNoLongerExists;

  /// No description provided for @thereIsNothingToTalkAbout.
  ///
  /// In en, this message translates to:
  /// **'There is nothing to talk about yet: no transcript, summary, notes or files.'**
  String get thereIsNothingToTalkAbout;

  /// No description provided for @theAiConnectionSContextIs.
  ///
  /// In en, this message translates to:
  /// **'The AI connection\'s context is too small. Choose one with a larger context.'**
  String get theAiConnectionSContextIs;

  /// No description provided for @thatMeetingNoLongerExists.
  ///
  /// In en, this message translates to:
  /// **'That meeting no longer exists.'**
  String get thatMeetingNoLongerExists;

  /// No description provided for @thatProjectNoLongerExists.
  ///
  /// In en, this message translates to:
  /// **'That project no longer exists.'**
  String get thatProjectNoLongerExists;

  /// No description provided for @theAiServerCouldNotBe.
  ///
  /// In en, this message translates to:
  /// **'The AI server could not be reached. Check the connection.'**
  String get theAiServerCouldNotBe;

  /// No description provided for @theAiServerRefusedTheApi.
  ///
  /// In en, this message translates to:
  /// **'The AI server refused the API key.'**
  String get theAiServerRefusedTheApi;

  /// No description provided for @theAiServerIsBusyTry.
  ///
  /// In en, this message translates to:
  /// **'The AI server is busy. Try again later.'**
  String get theAiServerIsBusyTry;

  /// No description provided for @theModelDeclinedToAnswer.
  ///
  /// In en, this message translates to:
  /// **'The model declined to answer.'**
  String get theModelDeclinedToAnswer;

  /// No description provided for @claudeCodeOrCodexIsNot.
  ///
  /// In en, this message translates to:
  /// **'Claude Code or Codex is not installed on this computer, or the app cannot find it.'**
  String get claudeCodeOrCodexIsNot;

  /// No description provided for @theNotesCouldNotBeRead.
  ///
  /// In en, this message translates to:
  /// **'The notes could not be read.'**
  String get theNotesCouldNotBeRead;

  /// No description provided for @noPairedComputerAnsweredOnA.
  ///
  /// In en, this message translates to:
  /// **'No paired computer answered. On a computer, open Settings → AI and choose a connection under \"Summaries for my other devices\", then keep it on and connected.'**
  String get noPairedComputerAnsweredOnA;

  /// No description provided for @theComputerTookTooLongTo.
  ///
  /// In en, this message translates to:
  /// **'The computer took too long to answer.'**
  String get theComputerTookTooLongTo;

  /// No description provided for @theComputerDisconnectedBeforeAnsweringTry.
  ///
  /// In en, this message translates to:
  /// **'The computer disconnected before answering. Try again.'**
  String get theComputerDisconnectedBeforeAnsweringTry;

  /// No description provided for @theComputerAnswersAtMost30.
  ///
  /// In en, this message translates to:
  /// **'The computer answers at most 30 questions an hour. Try again later.'**
  String get theComputerAnswersAtMost30;

  /// No description provided for @theComputerIsAnsweringOtherQuestions.
  ///
  /// In en, this message translates to:
  /// **'The computer is answering other questions. Try again shortly.'**
  String get theComputerIsAnsweringOtherQuestions;

  /// No description provided for @pairedDevicesAreOffOnThis.
  ///
  /// In en, this message translates to:
  /// **'Paired devices are off on this phone.'**
  String get pairedDevicesAreOffOnThis;

  /// No description provided for @theAiServerCouldNotAnswer.
  ///
  /// In en, this message translates to:
  /// **'The AI server could not answer. Try again.'**
  String get theAiServerCouldNotAnswer;

  /// No description provided for @exportMeeting.
  ///
  /// In en, this message translates to:
  /// **'Export meeting'**
  String get exportMeeting;

  /// No description provided for @openaiCompatibleServer.
  ///
  /// In en, this message translates to:
  /// **'OpenAI-compatible server'**
  String get openaiCompatibleServer;

  /// No description provided for @localServerOllama.
  ///
  /// In en, this message translates to:
  /// **'Local server (Ollama)'**
  String get localServerOllama;

  /// No description provided for @claudeCodeClaudePlan.
  ///
  /// In en, this message translates to:
  /// **'Claude Code (Claude plan)'**
  String get claudeCodeClaudePlan;

  /// No description provided for @codexChatgptPlan.
  ///
  /// In en, this message translates to:
  /// **'Codex (ChatGPT plan)'**
  String get codexChatgptPlan;

  /// No description provided for @checkTheAddressAndModelPlain.
  ///
  /// In en, this message translates to:
  /// **'Check the address and model. Plain http:// works only on this device or the local network.'**
  String get checkTheAddressAndModelPlain;

  /// No description provided for @thatConnectionNoLongerExists.
  ///
  /// In en, this message translates to:
  /// **'That connection no longer exists.'**
  String get thatConnectionNoLongerExists;

  /// No description provided for @addAnAiConnectionInSettings2.
  ///
  /// In en, this message translates to:
  /// **'Add an AI connection in Settings first.'**
  String get addAnAiConnectionInSettings2;

  /// No description provided for @thisMeetingHasNoTranscriptOr.
  ///
  /// In en, this message translates to:
  /// **'This meeting has no transcript or notes yet.'**
  String get thisMeetingHasNoTranscriptOr;

  /// No description provided for @thisMeetingIsTooLongFor.
  ///
  /// In en, this message translates to:
  /// **'This meeting is too long for this connection. Choose one with a larger context.'**
  String get thisMeetingIsTooLongFor;

  /// No description provided for @theAiServerCouldNotBe2.
  ///
  /// In en, this message translates to:
  /// **'The AI server could not be reached.'**
  String get theAiServerCouldNotBe2;

  /// No description provided for @theAiServerRefusedAccessUse.
  ///
  /// In en, this message translates to:
  /// **'The AI server refused access. Use a server that needs no API key.'**
  String get theAiServerRefusedAccessUse;

  /// No description provided for @theAiServerIsBusyOr.
  ///
  /// In en, this message translates to:
  /// **'The AI server is busy or over its limit. Try again later.'**
  String get theAiServerIsBusyOr;

  /// No description provided for @theAiServerRejectedTheRequest2.
  ///
  /// In en, this message translates to:
  /// **'The AI server rejected the request. Check the model name.'**
  String get theAiServerRejectedTheRequest2;

  /// No description provided for @theAiServerFailedTryAgain.
  ///
  /// In en, this message translates to:
  /// **'The AI server failed. Try again.'**
  String get theAiServerFailedTryAgain;

  /// No description provided for @theAiServerSAnswerCould.
  ///
  /// In en, this message translates to:
  /// **'The AI server\'s answer could not be read.'**
  String get theAiServerSAnswerCould;

  /// No description provided for @claudeCodeOrCodexCouldNot2.
  ///
  /// In en, this message translates to:
  /// **'Claude Code or Codex could not answer. Check that it is signed in.'**
  String get claudeCodeOrCodexCouldNot2;

  /// No description provided for @theSettingsCouldNotBeSaved.
  ///
  /// In en, this message translates to:
  /// **'The settings could not be saved.'**
  String get theSettingsCouldNotBeSaved;

  /// No description provided for @theMeetingCouldNotBeRead.
  ///
  /// In en, this message translates to:
  /// **'The meeting could not be read or saved.'**
  String get theMeetingCouldNotBeRead;

  /// No description provided for @error.
  ///
  /// In en, this message translates to:
  /// **'Error'**
  String get error;

  /// No description provided for @warning.
  ///
  /// In en, this message translates to:
  /// **'Warning'**
  String get warning;

  /// No description provided for @infoDefault.
  ///
  /// In en, this message translates to:
  /// **'Info (default)'**
  String get infoDefault;

  /// No description provided for @debug.
  ///
  /// In en, this message translates to:
  /// **'Debug'**
  String get debug;

  /// No description provided for @trace.
  ///
  /// In en, this message translates to:
  /// **'Trace'**
  String get trace;

  /// No description provided for @thisDevice2.
  ///
  /// In en, this message translates to:
  /// **'this device'**
  String get thisDevice2;

  /// No description provided for @anotherDevice.
  ///
  /// In en, this message translates to:
  /// **'another device'**
  String get anotherDevice;

  /// No description provided for @notConnected.
  ///
  /// In en, this message translates to:
  /// **'{label} (not connected)'**
  String notConnected(Object label);

  /// No description provided for @itIsKeptEncryptedUntil.
  ///
  /// In en, this message translates to:
  /// **' It is kept encrypted until {until}.'**
  String itIsKeptEncryptedUntil(Object until);

  /// No description provided for @recordedOnThisDeviceOnly.
  ///
  /// In en, this message translates to:
  /// **'Recorded on this device only.'**
  String get recordedOnThisDeviceOnly;

  /// No description provided for @recordedOnThisDeviceOnlyA.
  ///
  /// In en, this message translates to:
  /// **'Recorded on this device only. A recording of it made on another device can still be combined until {until}.'**
  String recordedOnThisDeviceOnlyA(Object until);

  /// No description provided for @combinesTheRecordingsNothingIsLeft.
  ///
  /// In en, this message translates to:
  /// **'{combiner} combines the recordings; nothing is left to send from here.'**
  String combinesTheRecordingsNothingIsLeft(Object combiner);

  /// No description provided for @waitingForAudioFrom.
  ///
  /// In en, this message translates to:
  /// **'Waiting for audio from {names}.{kept}'**
  String waitingForAudioFrom(Object names, Object kept);

  /// No description provided for @combinesTheRecordingsWaitingForAudio.
  ///
  /// In en, this message translates to:
  /// **'{combiner} combines the recordings; waiting for audio from {names}.'**
  String combinesTheRecordingsWaitingForAudio(Object combiner, Object names);

  /// No description provided for @audioFromEveryDeviceIsHere.
  ///
  /// In en, this message translates to:
  /// **'Audio from every device is here; the transcript is combined when this device is free.'**
  String get audioFromEveryDeviceIsHere;

  /// No description provided for @thisDeviceSAudioStillHas.
  ///
  /// In en, this message translates to:
  /// **'This device\'s audio still has to reach {combiner}.{kept}'**
  String thisDeviceSAudioStillHas(Object combiner, Object kept);

  /// No description provided for @transcriptCombinedFromEveryDevice.
  ///
  /// In en, this message translates to:
  /// **'Transcript combined from every device.'**
  String get transcriptCombinedFromEveryDevice;

  /// No description provided for @notCombinedAudioFromDidNot.
  ///
  /// In en, this message translates to:
  /// **'Not combined: audio from {plainNames} did not arrive within three weeks.'**
  String notCombinedAudioFromDidNot(Object plainNames);

  /// No description provided for @meeting.
  ///
  /// In en, this message translates to:
  /// **'Meeting {createdAt}'**
  String meeting(Object createdAt);

  /// No description provided for @writeSomethingFirst.
  ///
  /// In en, this message translates to:
  /// **'Write something first.'**
  String get writeSomethingFirst;

  /// No description provided for @theTextIsTooLong.
  ///
  /// In en, this message translates to:
  /// **'The text is too long.'**
  String get theTextIsTooLong;

  /// No description provided for @thatPartOfTheTranscriptChanged.
  ///
  /// In en, this message translates to:
  /// **'That part of the transcript changed meanwhile. Reopen the meeting.'**
  String get thatPartOfTheTranscriptChanged;

  /// No description provided for @theMeetingCouldNotBeSaved.
  ///
  /// In en, this message translates to:
  /// **'The meeting could not be saved.'**
  String get theMeetingCouldNotBeSaved;

  /// No description provided for @chooseAFullFolderPathInside.
  ///
  /// In en, this message translates to:
  /// **'Choose a full folder path, inside your home folder but not the home folder itself.'**
  String get chooseAFullFolderPathInside;

  /// No description provided for @theFolderCouldNotBeWritten.
  ///
  /// In en, this message translates to:
  /// **'The folder could not be written. Check that it exists and you can write there.'**
  String get theFolderCouldNotBeWritten;

  /// No description provided for @theNotesCouldNotBeRead2.
  ///
  /// In en, this message translates to:
  /// **'The notes could not be read or saved.'**
  String get theNotesCouldNotBeRead2;

  /// No description provided for @theInviteAnswerIsIncomplete.
  ///
  /// In en, this message translates to:
  /// **'The invite answer is incomplete.'**
  String get theInviteAnswerIsIncomplete;

  /// No description provided for @theRenameAnswerIsIncomplete.
  ///
  /// In en, this message translates to:
  /// **'The rename answer is incomplete.'**
  String get theRenameAnswerIsIncomplete;

  /// No description provided for @thisDeviceHasNoCamera.
  ///
  /// In en, this message translates to:
  /// **'This device has no camera.'**
  String get thisDeviceHasNoCamera;

  /// No description provided for @theCameraIsNotOpen.
  ///
  /// In en, this message translates to:
  /// **'The camera is not open.'**
  String get theCameraIsNotOpen;

  /// No description provided for @title.
  ///
  /// In en, this message translates to:
  /// **'Title'**
  String get title;

  /// No description provided for @summary.
  ///
  /// In en, this message translates to:
  /// **'Summary'**
  String get summary;

  /// No description provided for @notes.
  ///
  /// In en, this message translates to:
  /// **'Notes'**
  String get notes;

  /// No description provided for @transcript.
  ///
  /// In en, this message translates to:
  /// **'Transcript'**
  String get transcript;

  /// No description provided for @projectName.
  ///
  /// In en, this message translates to:
  /// **'Project name'**
  String get projectName;

  /// No description provided for @file.
  ///
  /// In en, this message translates to:
  /// **'File {fileName}'**
  String file(Object fileName);

  /// No description provided for @typeWhatToLookFor.
  ///
  /// In en, this message translates to:
  /// **'Type what to look for.'**
  String get typeWhatToLookFor;

  /// No description provided for @typeAtLeastTwoLetters.
  ///
  /// In en, this message translates to:
  /// **'Type at least two letters.'**
  String get typeAtLeastTwoLetters;

  /// No description provided for @theSearchIsTooLong.
  ///
  /// In en, this message translates to:
  /// **'The search is too long.'**
  String get theSearchIsTooLong;

  /// No description provided for @exportLogs.
  ///
  /// In en, this message translates to:
  /// **'Export logs'**
  String get exportLogs;

  /// No description provided for @about.
  ///
  /// In en, this message translates to:
  /// **'About {appName}'**
  String about(Object appName);

  /// No description provided for @versionLicenseAndTheSoftwareIt.
  ///
  /// In en, this message translates to:
  /// **'Version, license and the software it uses.'**
  String get versionLicenseAndTheSoftwareIt;

  /// No description provided for @remove.
  ///
  /// In en, this message translates to:
  /// **'Remove {label}?'**
  String remove(Object label);

  /// No description provided for @itsApiKeyIsDeletedFrom.
  ///
  /// In en, this message translates to:
  /// **'Its API key is deleted from this device. Summaries already written stay.'**
  String get itsApiKeyIsDeletedFrom;

  /// No description provided for @cancel.
  ///
  /// In en, this message translates to:
  /// **'Cancel'**
  String get cancel;

  /// No description provided for @remove2.
  ///
  /// In en, this message translates to:
  /// **'Remove'**
  String get remove2;

  /// No description provided for @aiConnections.
  ///
  /// In en, this message translates to:
  /// **'AI connections'**
  String get aiConnections;

  /// No description provided for @addConnection.
  ///
  /// In en, this message translates to:
  /// **'Add connection'**
  String get addConnection;

  /// No description provided for @summariesSendTheMeetingSTranscript.
  ///
  /// In en, this message translates to:
  /// **'Summaries send the meeting\'s transcript, notes and photos to the connection\'s server. A server on your local network keeps them there.'**
  String get summariesSendTheMeetingSTranscript;

  /// No description provided for @noConnectionsYet.
  ///
  /// In en, this message translates to:
  /// **'No connections yet'**
  String get noConnectionsYet;

  /// No description provided for @addAServerOnYourNetwork.
  ///
  /// In en, this message translates to:
  /// **'Add a server on your network (Ollama, LM Studio, llama.cpp), or Claude Code or Codex on this computer.'**
  String get addAServerOnYourNetwork;

  /// No description provided for @summarizeMeetingsAutomatically.
  ///
  /// In en, this message translates to:
  /// **'Summarize meetings automatically'**
  String get summarizeMeetingsAutomatically;

  /// No description provided for @withTheDefaultConnectionWhenA.
  ///
  /// In en, this message translates to:
  /// **'With the default connection, when a meeting\'s transcript is final.'**
  String get withTheDefaultConnectionWhenA;

  /// No description provided for @summariesForMyOtherDevices.
  ///
  /// In en, this message translates to:
  /// **'Summaries for my other devices'**
  String get summariesForMyOtherDevices;

  /// No description provided for @aPairedPhoneCanAskThis.
  ///
  /// In en, this message translates to:
  /// **'A paired phone can ask this computer to write a summary or answer a chat with this connection, from this computer\'s copy of your notes. It counts against that plan; at most 10 summaries and 30 chat answers an hour.'**
  String get aPairedPhoneCanAskThis;

  /// No description provided for @off.
  ///
  /// In en, this message translates to:
  /// **'Off'**
  String get off;

  /// No description provided for @summaryPrompt.
  ///
  /// In en, this message translates to:
  /// **'Summary prompt'**
  String get summaryPrompt;

  /// No description provided for @placeholdersTheTranscriptIsAlwaysSent.
  ///
  /// In en, this message translates to:
  /// **'Placeholders: {placeholders}. The transcript is always sent after the prompt.'**
  String placeholdersTheTranscriptIsAlwaysSent(Object placeholders);

  /// No description provided for @savePrompt.
  ///
  /// In en, this message translates to:
  /// **'Save prompt'**
  String get savePrompt;

  /// No description provided for @resetToBuiltIn.
  ///
  /// In en, this message translates to:
  /// **'Reset to built-in'**
  String get resetToBuiltIn;

  /// No description provided for @defaultLabel.
  ///
  /// In en, this message translates to:
  /// **'Default'**
  String get defaultLabel;

  /// No description provided for @installed.
  ///
  /// In en, this message translates to:
  /// **'Installed'**
  String get installed;

  /// No description provided for @notFoundOnThisComputer.
  ///
  /// In en, this message translates to:
  /// **'Not found on this computer'**
  String get notFoundOnThisComputer;

  /// No description provided for @unencryptedHttpLocalNetworkOnly.
  ///
  /// In en, this message translates to:
  /// **'Unencrypted http, local network only'**
  String get unencryptedHttpLocalNetworkOnly;

  /// No description provided for @runsWithYourPlanSSign.
  ///
  /// In en, this message translates to:
  /// **'Runs with your plan\'s sign-in on this computer'**
  String get runsWithYourPlanSSign;

  /// No description provided for @actionsFor.
  ///
  /// In en, this message translates to:
  /// **'Actions for {label}'**
  String actionsFor(Object label);

  /// No description provided for @makeDefault.
  ///
  /// In en, this message translates to:
  /// **'Make default'**
  String get makeDefault;

  /// No description provided for @test.
  ///
  /// In en, this message translates to:
  /// **'Test'**
  String get test;

  /// No description provided for @edit.
  ///
  /// In en, this message translates to:
  /// **'Edit'**
  String get edit;

  /// No description provided for @enterANameAnAddressAnd.
  ///
  /// In en, this message translates to:
  /// **'Enter a name, an address and a model.'**
  String get enterANameAnAddressAnd;

  /// No description provided for @theContextIsANumberOf.
  ///
  /// In en, this message translates to:
  /// **'The context is a number of tokens, such as 8192.'**
  String get theContextIsANumberOf;

  /// No description provided for @edit2.
  ///
  /// In en, this message translates to:
  /// **'Edit {label}'**
  String edit2(Object label);

  /// No description provided for @name.
  ///
  /// In en, this message translates to:
  /// **'Name'**
  String get name;

  /// No description provided for @protocol.
  ///
  /// In en, this message translates to:
  /// **'Protocol'**
  String get protocol;

  /// No description provided for @runsTheToolInstalledAndSigned.
  ///
  /// In en, this message translates to:
  /// **'Runs the tool installed and signed in on this computer, so your Claude or ChatGPT plan writes the summaries instead of API credit. The meeting goes to Anthropic or OpenAI under that account.'**
  String get runsTheToolInstalledAndSigned;

  /// No description provided for @address.
  ///
  /// In en, this message translates to:
  /// **'Address'**
  String get address;

  /// No description provided for @httpsAlwaysHttpOnlyOnThis.
  ///
  /// In en, this message translates to:
  /// **'https:// always; http:// only on this device or the local network.'**
  String get httpsAlwaysHttpOnlyOnThis;

  /// No description provided for @model.
  ///
  /// In en, this message translates to:
  /// **'Model'**
  String get model;

  /// No description provided for @leaveEmptyForTheToolS.
  ///
  /// In en, this message translates to:
  /// **'Leave empty for the tool\'s own default.'**
  String get leaveEmptyForTheToolS;

  /// No description provided for @contextTokens.
  ///
  /// In en, this message translates to:
  /// **'Context (tokens)'**
  String get contextTokens;

  /// No description provided for @saveConnection.
  ///
  /// In en, this message translates to:
  /// **'Save connection'**
  String get saveConnection;

  /// No description provided for @koalinoIsLocked.
  ///
  /// In en, this message translates to:
  /// **'Koalino is locked'**
  String get koalinoIsLocked;

  /// No description provided for @unlockWithYourFingerprintFaceOr.
  ///
  /// In en, this message translates to:
  /// **'Unlock with your fingerprint, face, or screen lock.'**
  String get unlockWithYourFingerprintFaceOr;

  /// No description provided for @unlock.
  ///
  /// In en, this message translates to:
  /// **'Unlock'**
  String get unlock;

  /// No description provided for @copied.
  ///
  /// In en, this message translates to:
  /// **'{what} copied'**
  String copied(Object what);

  /// No description provided for @aiAssistantAccess.
  ///
  /// In en, this message translates to:
  /// **'AI assistant access'**
  String get aiAssistantAccess;

  /// No description provided for @letYourOwnAiAssistantSuch.
  ///
  /// In en, this message translates to:
  /// **'Let your own AI assistant, such as Claude Code, Codex or Claude Desktop, read your meetings and projects: titles, summaries, notes, transcripts and project files. It can only read; it cannot change or delete anything.'**
  String get letYourOwnAiAssistantSuch;

  /// No description provided for @whatTheAssistantReadsMayBe.
  ///
  /// In en, this message translates to:
  /// **'What the assistant reads may be sent to its online model, under your account with that service. Turn this on only for an assistant you trust with your meetings.'**
  String get whatTheAssistantReadsMayBe;

  /// No description provided for @theAssistantServerIsNotInstalled.
  ///
  /// In en, this message translates to:
  /// **'The assistant server is not installed with this copy of the app.'**
  String get theAssistantServerIsNotInstalled;

  /// No description provided for @letMyAiAssistantReadMy.
  ///
  /// In en, this message translates to:
  /// **'Let my AI assistant read my notes'**
  String get letMyAiAssistantReadMy;

  /// No description provided for @connectYourAssistant.
  ///
  /// In en, this message translates to:
  /// **'Connect your assistant'**
  String get connectYourAssistant;

  /// No description provided for @yourNotesAreUnlockedWithA.
  ///
  /// In en, this message translates to:
  /// **'Your notes are unlocked with a passphrase, which is never written into an assistant\'s settings. The server reads them only when NOTE_TAKER_ENCRYPTION_PASSPHRASE is set where the assistant starts it.'**
  String get yourNotesAreUnlockedWithA;

  /// No description provided for @claudeCodeRunInATerminal.
  ///
  /// In en, this message translates to:
  /// **'Claude Code: run in a terminal'**
  String get claudeCodeRunInATerminal;

  /// No description provided for @command.
  ///
  /// In en, this message translates to:
  /// **'Command'**
  String get command;

  /// No description provided for @claudeDesktopAndOtherMcpClients.
  ///
  /// In en, this message translates to:
  /// **'Claude Desktop and other MCP clients: add to their settings'**
  String get claudeDesktopAndOtherMcpClients;

  /// No description provided for @settings.
  ///
  /// In en, this message translates to:
  /// **'Settings'**
  String get settings;

  /// No description provided for @copy.
  ///
  /// In en, this message translates to:
  /// **'Copy: {title}'**
  String copy(Object title);

  /// No description provided for @isUsingTheMicrophoneRecordThis.
  ///
  /// In en, this message translates to:
  /// **'{app} is using the microphone. Record this call?'**
  String isUsingTheMicrophoneRecordThis(Object app);

  /// No description provided for @notNow.
  ///
  /// In en, this message translates to:
  /// **'Not now'**
  String get notNow;

  /// No description provided for @record.
  ///
  /// In en, this message translates to:
  /// **'Record'**
  String get record;

  /// No description provided for @offerToRecordCalls.
  ///
  /// In en, this message translates to:
  /// **'Offer to record calls'**
  String get offerToRecordCalls;

  /// No description provided for @whenTeamsZoomMeetInA.
  ///
  /// In en, this message translates to:
  /// **'When Teams, Zoom, Meet in a browser or another call app starts using the microphone. Nothing records until you tap Record.'**
  String get whenTeamsZoomMeetInA;

  /// No description provided for @stopWhenTheCallEnds.
  ///
  /// In en, this message translates to:
  /// **'Stop when the call ends'**
  String get stopWhenTheCallEnds;

  /// No description provided for @onlyForRecordingsStartedFromThe.
  ///
  /// In en, this message translates to:
  /// **'Only for recordings started from the offer, once the call app has left the microphone for 20 seconds.'**
  String get onlyForRecordingsStartedFromThe;

  /// No description provided for @readSpeakerNamesFromTheCall.
  ///
  /// In en, this message translates to:
  /// **'Read speaker names from the call window'**
  String get readSpeakerNamesFromTheCall;

  /// No description provided for @whileRecordingKoalinoReadsWhoTeams.
  ///
  /// In en, this message translates to:
  /// **'While recording, Koalino reads who Teams, Zoom or Meet shows as speaking, to suggest names for unknown voices. Nothing leaves this computer, and nothing is named until you tap Assign.'**
  String get whileRecordingKoalinoReadsWhoTeams;

  /// No description provided for @deleteThisChat.
  ///
  /// In en, this message translates to:
  /// **'Delete this chat?'**
  String get deleteThisChat;

  /// No description provided for @itsQuestionsAndAnswersAreDeleted.
  ///
  /// In en, this message translates to:
  /// **'Its questions and answers are deleted from this device.'**
  String get itsQuestionsAndAnswersAreDeleted;

  /// No description provided for @delete.
  ///
  /// In en, this message translates to:
  /// **'Delete'**
  String get delete;

  /// No description provided for @thatFileIsNoLongerIn.
  ///
  /// In en, this message translates to:
  /// **'That file is no longer in the project.'**
  String get thatFileIsNoLongerIn;

  /// No description provided for @whoAnswers.
  ///
  /// In en, this message translates to:
  /// **'Who answers'**
  String get whoAnswers;

  /// No description provided for @myPairedComputer.
  ///
  /// In en, this message translates to:
  /// **'My paired computer'**
  String get myPairedComputer;

  /// No description provided for @thisPhoneSAiConnection.
  ///
  /// In en, this message translates to:
  /// **'This phone\'s AI connection'**
  String get thisPhoneSAiConnection;

  /// No description provided for @deleteThisChat2.
  ///
  /// In en, this message translates to:
  /// **'Delete this chat'**
  String get deleteThisChat2;

  /// No description provided for @askAboutYourNotesAnswersCome.
  ///
  /// In en, this message translates to:
  /// **'Ask about your notes. Answers come from your paired computer, which reads its own copy of your notes with the connection chosen there for your other devices.'**
  String get askAboutYourNotesAnswersCome;

  /// No description provided for @askAboutYourNotesTheAnswer.
  ///
  /// In en, this message translates to:
  /// **'Ask about your notes. The answer comes from your default AI connection, which is sent the notes that fit.'**
  String get askAboutYourNotesTheAnswer;

  /// No description provided for @yourQuestion.
  ///
  /// In en, this message translates to:
  /// **'Your question'**
  String get yourQuestion;

  /// No description provided for @ask.
  ///
  /// In en, this message translates to:
  /// **'Ask'**
  String get ask;

  /// No description provided for @open.
  ///
  /// In en, this message translates to:
  /// **'Open'**
  String get open;

  /// No description provided for @given.
  ///
  /// In en, this message translates to:
  /// **'Given: {included}'**
  String given(Object included);

  /// No description provided for @leftOutForLength.
  ///
  /// In en, this message translates to:
  /// **'Left out for length: {omitted}'**
  String leftOutForLength(Object omitted);

  /// No description provided for @answeredBy.
  ///
  /// In en, this message translates to:
  /// **'Answered by {connection}'**
  String answeredBy(Object connection);

  /// No description provided for @unableToOpenTheKeychain.
  ///
  /// In en, this message translates to:
  /// **'Unable to open the keychain'**
  String get unableToOpenTheKeychain;

  /// No description provided for @koalinoKeepsTheKeyThatEncrypts.
  ///
  /// In en, this message translates to:
  /// **'Koalino keeps the key that encrypts your voice profiles and device identity in the system keychain. Unlock the keychain, or allow Koalino to use it, then try again. If this device has no keychain service, install or start one and restart Koalino.'**
  String get koalinoKeepsTheKeyThatEncrypts;

  /// No description provided for @unableToOpenYourNotes.
  ///
  /// In en, this message translates to:
  /// **'Unable to open your notes'**
  String get unableToOpenYourNotes;

  /// No description provided for @koalinoCouldNotOpenItsDatabase.
  ///
  /// In en, this message translates to:
  /// **'Koalino could not open its database. Check that there is free space and try again.'**
  String get koalinoCouldNotOpenItsDatabase;

  /// No description provided for @unlockingYourNotes.
  ///
  /// In en, this message translates to:
  /// **'Unlocking your notes…'**
  String get unlockingYourNotes;

  /// No description provided for @quit.
  ///
  /// In en, this message translates to:
  /// **'Quit'**
  String get quit;

  /// No description provided for @retry.
  ///
  /// In en, this message translates to:
  /// **'Retry'**
  String get retry;

  /// No description provided for @delete2.
  ///
  /// In en, this message translates to:
  /// **'Delete {title}?'**
  String delete2(Object title);

  /// No description provided for @itsTranscriptAttachmentsAndUnknownVoices.
  ///
  /// In en, this message translates to:
  /// **'Its transcript, attachments and unknown voices go for good, here and on your paired devices. People you saved stay.'**
  String get itsTranscriptAttachmentsAndUnknownVoices;

  /// No description provided for @unableToDeleteThisMeetingIf.
  ///
  /// In en, this message translates to:
  /// **'Unable to delete this meeting. If it is being recorded, stop the recording first.'**
  String get unableToDeleteThisMeetingIf;

  /// No description provided for @devices.
  ///
  /// In en, this message translates to:
  /// **'Devices'**
  String get devices;

  /// No description provided for @deviceSyncIsOff.
  ///
  /// In en, this message translates to:
  /// **'Device sync is off'**
  String get deviceSyncIsOff;

  /// No description provided for @meetingsStayOnThisDeviceUntil.
  ///
  /// In en, this message translates to:
  /// **'Meetings stay on this device until sync is turned on.'**
  String get meetingsStayOnThisDeviceUntil;

  /// No description provided for @thisDevice3.
  ///
  /// In en, this message translates to:
  /// **'This device\n{capabilities}'**
  String thisDevice3(Object capabilities);

  /// No description provided for @renameThisDevice.
  ///
  /// In en, this message translates to:
  /// **'Rename this device'**
  String get renameThisDevice;

  /// No description provided for @discoverableYourOtherDevicesCanAsk.
  ///
  /// In en, this message translates to:
  /// **'Discoverable: your other devices can ask to pair for a few minutes.'**
  String get discoverableYourOtherDevicesCanAsk;

  /// No description provided for @makeDiscoverable.
  ///
  /// In en, this message translates to:
  /// **'Make discoverable'**
  String get makeDiscoverable;

  /// No description provided for @pairingRequests.
  ///
  /// In en, this message translates to:
  /// **'Pairing requests'**
  String get pairingRequests;

  /// No description provided for @acceptOnlyIfTheOtherDevice.
  ///
  /// In en, this message translates to:
  /// **'Accept only if the other device shows the same code.'**
  String get acceptOnlyIfTheOtherDevice;

  /// No description provided for @reject.
  ///
  /// In en, this message translates to:
  /// **'Reject'**
  String get reject;

  /// No description provided for @accept.
  ///
  /// In en, this message translates to:
  /// **'Accept'**
  String get accept;

  /// No description provided for @pairingInProgress.
  ///
  /// In en, this message translates to:
  /// **'Pairing in progress'**
  String get pairingInProgress;

  /// No description provided for @waitingForTheOtherDevice.
  ///
  /// In en, this message translates to:
  /// **'Waiting for the other device…'**
  String get waitingForTheOtherDevice;

  /// No description provided for @checkThatTheOtherDeviceShows.
  ///
  /// In en, this message translates to:
  /// **'Check that the other device shows the same code.'**
  String get checkThatTheOtherDeviceShows;

  /// No description provided for @cancelPairing.
  ///
  /// In en, this message translates to:
  /// **'Cancel pairing'**
  String get cancelPairing;

  /// No description provided for @codesMatch.
  ///
  /// In en, this message translates to:
  /// **'Codes match'**
  String get codesMatch;

  /// No description provided for @pairedDevices.
  ///
  /// In en, this message translates to:
  /// **'Paired devices'**
  String get pairedDevices;

  /// No description provided for @noPairedDevicesYet.
  ///
  /// In en, this message translates to:
  /// **'No paired devices yet.'**
  String get noPairedDevicesYet;

  /// No description provided for @paired.
  ///
  /// In en, this message translates to:
  /// **'Paired {pairedAt}\n{hasRelay}'**
  String paired(Object pairedAt, Object hasRelay);

  /// No description provided for @remove3.
  ///
  /// In en, this message translates to:
  /// **'Remove {deviceName}'**
  String remove3(Object deviceName);

  /// No description provided for @nearbyDevices.
  ///
  /// In en, this message translates to:
  /// **'Nearby devices'**
  String get nearbyDevices;

  /// No description provided for @pair.
  ///
  /// In en, this message translates to:
  /// **'Pair'**
  String get pair;

  /// No description provided for @anotherNetwork.
  ///
  /// In en, this message translates to:
  /// **'Another network'**
  String get anotherNetwork;

  /// No description provided for @showAnInviteHereAndUse.
  ///
  /// In en, this message translates to:
  /// **'Show an invite here and use it on the other device. You still compare a code before they pair.'**
  String get showAnInviteHereAndUse;

  /// No description provided for @showInvite.
  ///
  /// In en, this message translates to:
  /// **'Show invite'**
  String get showInvite;

  /// No description provided for @useInvite.
  ///
  /// In en, this message translates to:
  /// **'Use invite'**
  String get useInvite;

  /// No description provided for @stopsSyncingWithThisDevicePair.
  ///
  /// In en, this message translates to:
  /// **'{deviceName} stops syncing with this device. Pair again to resume.'**
  String stopsSyncingWithThisDevicePair(Object deviceName);

  /// No description provided for @pairingCode.
  ///
  /// In en, this message translates to:
  /// **'Pairing code {split}'**
  String pairingCode(Object split);

  /// No description provided for @noOtherDevicesFoundOnThis.
  ///
  /// In en, this message translates to:
  /// **'No other devices found on this network. Nearby only finds devices on the same Wi-Fi. For a device on another network, use an invite.'**
  String get noOtherDevicesFoundOnThis;

  /// No description provided for @noOtherDevicesFoundOnThis2.
  ///
  /// In en, this message translates to:
  /// **'No other devices found on this network. Nearby only finds devices on the same Wi-Fi. For a device on another network, use an invite or set a relay in Settings.'**
  String get noOtherDevicesFoundOnThis2;

  /// No description provided for @notConnectedItReconnectsWhenIt.
  ///
  /// In en, this message translates to:
  /// **'Not connected. It reconnects when it is online and can reach your relay.'**
  String get notConnectedItReconnectsWhenIt;

  /// No description provided for @notConnectedPutBothDevicesOn.
  ///
  /// In en, this message translates to:
  /// **'Not connected. Put both devices on the same Wi-Fi, or set a relay in Settings.'**
  String get notConnectedPutBothDevicesOn;

  /// No description provided for @whisperOnDevice.
  ///
  /// In en, this message translates to:
  /// **'Whisper (on device)'**
  String get whisperOnDevice;

  /// No description provided for @openaiCloud.
  ///
  /// In en, this message translates to:
  /// **'OpenAI (cloud)'**
  String get openaiCloud;

  /// No description provided for @gbGpu.
  ///
  /// In en, this message translates to:
  /// **'{round} GB GPU'**
  String gbGpu(Object round);

  /// No description provided for @mbGpu.
  ///
  /// In en, this message translates to:
  /// **'{gpuMegabytes} MB GPU'**
  String mbGpu(Object gpuMegabytes);

  /// No description provided for @noTranscriptionModels.
  ///
  /// In en, this message translates to:
  /// **'No transcription models'**
  String get noTranscriptionModels;

  /// No description provided for @useYourOwnAiApp.
  ///
  /// In en, this message translates to:
  /// **'Use your own AI app'**
  String get useYourOwnAiApp;

  /// No description provided for @theMeetingItsNotesAndIts.
  ///
  /// In en, this message translates to:
  /// **'The meeting, its notes and its files go to the app you pick, with your own account. For a summary, copy the answer there and come back, or share it to Koalino: Koalino keeps it.'**
  String get theMeetingItsNotesAndIts;

  /// No description provided for @summarizeIn.
  ///
  /// In en, this message translates to:
  /// **'Summarize in {displayName}'**
  String summarizeIn(Object displayName);

  /// No description provided for @chatIn.
  ///
  /// In en, this message translates to:
  /// **'Chat in {displayName}'**
  String chatIn(Object displayName);

  /// No description provided for @summarizeInAnotherApp.
  ///
  /// In en, this message translates to:
  /// **'Summarize in another app…'**
  String get summarizeInAnotherApp;

  /// No description provided for @chatInAnotherApp.
  ///
  /// In en, this message translates to:
  /// **'Chat in another app…'**
  String get chatInAnotherApp;

  /// No description provided for @summaryFromSavedTo.
  ///
  /// In en, this message translates to:
  /// **'Summary from {appName} saved to \"{title}\".'**
  String summaryFromSavedTo(Object appName, Object title);

  /// No description provided for @undo.
  ///
  /// In en, this message translates to:
  /// **'Undo'**
  String get undo;

  /// No description provided for @theEarlierSummaryIsBack.
  ///
  /// In en, this message translates to:
  /// **'The earlier summary is back.'**
  String get theEarlierSummaryIsBack;

  /// No description provided for @toKeepTheSummaryCopyS.
  ///
  /// In en, this message translates to:
  /// **'To keep the summary, copy {appName}\'s answer and come back to Koalino, or share the answer to Koalino.'**
  String toKeepTheSummaryCopyS(Object appName);

  /// No description provided for @meetings.
  ///
  /// In en, this message translates to:
  /// **'Meetings'**
  String get meetings;

  /// No description provided for @projects.
  ///
  /// In en, this message translates to:
  /// **'Projects'**
  String get projects;

  /// No description provided for @voices.
  ///
  /// In en, this message translates to:
  /// **'Voices'**
  String get voices;

  /// No description provided for @meeting2.
  ///
  /// In en, this message translates to:
  /// **'Meeting'**
  String get meeting2;

  /// No description provided for @chatAboutThisMeeting.
  ///
  /// In en, this message translates to:
  /// **'Chat about this meeting'**
  String get chatAboutThisMeeting;

  /// No description provided for @chat.
  ///
  /// In en, this message translates to:
  /// **'Chat: {meeting}'**
  String chat(Object meeting);

  /// No description provided for @thisMeeting.
  ///
  /// In en, this message translates to:
  /// **'this meeting'**
  String get thisMeeting;

  /// No description provided for @takePhotos.
  ///
  /// In en, this message translates to:
  /// **'Take photos'**
  String get takePhotos;

  /// No description provided for @attachFiles.
  ///
  /// In en, this message translates to:
  /// **'Attach files'**
  String get attachFiles;

  /// No description provided for @export.
  ///
  /// In en, this message translates to:
  /// **'Export'**
  String get export;

  /// No description provided for @shareAs.
  ///
  /// In en, this message translates to:
  /// **'Share as {label}'**
  String shareAs(Object label);

  /// No description provided for @nameSpeakers.
  ///
  /// In en, this message translates to:
  /// **'Name speakers'**
  String get nameSpeakers;

  /// No description provided for @selectAMeetingToSeeIts.
  ///
  /// In en, this message translates to:
  /// **'Select a meeting to see its transcript.'**
  String get selectAMeetingToSeeIts;

  /// No description provided for @renameMeeting.
  ///
  /// In en, this message translates to:
  /// **'Rename meeting'**
  String get renameMeeting;

  /// No description provided for @deleteMeeting.
  ///
  /// In en, this message translates to:
  /// **'Delete meeting'**
  String get deleteMeeting;

  /// No description provided for @recordedOn.
  ///
  /// In en, this message translates to:
  /// **'Recorded on {devices}'**
  String recordedOn(Object devices);

  /// No description provided for @split.
  ///
  /// In en, this message translates to:
  /// **'Split'**
  String get split;

  /// No description provided for @longPressOrRightClickA.
  ///
  /// In en, this message translates to:
  /// **'Long-press or right-click a passage to correct it.'**
  String get longPressOrRightClickA;

  /// No description provided for @noTranscriptIsAvailable.
  ///
  /// In en, this message translates to:
  /// **'No transcript is available.'**
  String get noTranscriptIsAvailable;

  /// No description provided for @theSpeakerIsAGuess.
  ///
  /// In en, this message translates to:
  /// **'The speaker is a guess'**
  String get theSpeakerIsAGuess;

  /// No description provided for @peopleTalkedAtOnceHereWords.
  ///
  /// In en, this message translates to:
  /// **'People talked at once here; words may be missing'**
  String get peopleTalkedAtOnceHereWords;

  /// No description provided for @overlappingSpeech.
  ///
  /// In en, this message translates to:
  /// **'Overlapping speech'**
  String get overlappingSpeech;

  /// No description provided for @someWordsWereUnclear.
  ///
  /// In en, this message translates to:
  /// **'Some words were unclear'**
  String get someWordsWereUnclear;

  /// No description provided for @addedAfterTheMeeting.
  ///
  /// In en, this message translates to:
  /// **'Added after the meeting'**
  String get addedAfterTheMeeting;

  /// No description provided for @photoOpenFullScreen.
  ///
  /// In en, this message translates to:
  /// **'Photo {fileName}, open full screen'**
  String photoOpenFullScreen(Object fileName);

  /// No description provided for @save.
  ///
  /// In en, this message translates to:
  /// **'Save'**
  String get save;

  /// No description provided for @preview.
  ///
  /// In en, this message translates to:
  /// **'Preview'**
  String get preview;

  /// No description provided for @editNotes.
  ///
  /// In en, this message translates to:
  /// **'Edit notes'**
  String get editNotes;

  /// No description provided for @writeNotes.
  ///
  /// In en, this message translates to:
  /// **'Write notes'**
  String get writeNotes;

  /// No description provided for @loading.
  ///
  /// In en, this message translates to:
  /// **'Loading…'**
  String get loading;

  /// No description provided for @noNotesYet.
  ///
  /// In en, this message translates to:
  /// **'No notes yet.'**
  String get noNotesYet;

  /// No description provided for @textAt.
  ///
  /// In en, this message translates to:
  /// **'Text at {startTimeMs}'**
  String textAt(Object startTimeMs);

  /// No description provided for @removedThisTranscriptWonTBe.
  ///
  /// In en, this message translates to:
  /// **'Removed. This transcript won\'t be re-transcribed after the meeting.'**
  String get removedThisTranscriptWonTBe;

  /// No description provided for @savedThisTranscriptWonTBe.
  ///
  /// In en, this message translates to:
  /// **'Saved. This transcript won\'t be re-transcribed after the meeting.'**
  String get savedThisTranscriptWonTBe;

  /// No description provided for @editText.
  ///
  /// In en, this message translates to:
  /// **'Edit text'**
  String get editText;

  /// No description provided for @editTheTranscript.
  ///
  /// In en, this message translates to:
  /// **'Edit the transcript'**
  String get editTheTranscript;

  /// No description provided for @leaveItEmptyToRemoveThis.
  ///
  /// In en, this message translates to:
  /// **'Leave it empty to remove this part.'**
  String get leaveItEmptyToRemoveThis;

  /// No description provided for @searchMeetings.
  ///
  /// In en, this message translates to:
  /// **'Search meetings'**
  String get searchMeetings;

  /// No description provided for @chatWithAllYourNotes.
  ///
  /// In en, this message translates to:
  /// **'Chat with all your notes'**
  String get chatWithAllYourNotes;

  /// No description provided for @chatWithAllNotes.
  ///
  /// In en, this message translates to:
  /// **'Chat with all notes'**
  String get chatWithAllNotes;

  /// No description provided for @refresh.
  ///
  /// In en, this message translates to:
  /// **'Refresh'**
  String get refresh;

  /// No description provided for @noMeetingsYet.
  ///
  /// In en, this message translates to:
  /// **'No meetings yet'**
  String get noMeetingsYet;

  /// No description provided for @recordingsYouStartAppearHere.
  ///
  /// In en, this message translates to:
  /// **'Recordings you start appear here.'**
  String get recordingsYouStartAppearHere;

  /// No description provided for @rename.
  ///
  /// In en, this message translates to:
  /// **'Rename'**
  String get rename;

  /// No description provided for @addToProject.
  ///
  /// In en, this message translates to:
  /// **'Add to project…'**
  String get addToProject;

  /// No description provided for @summary2.
  ///
  /// In en, this message translates to:
  /// **'Summary…'**
  String get summary2;

  /// No description provided for @addToProject2.
  ///
  /// In en, this message translates to:
  /// **'Add to project'**
  String get addToProject2;

  /// No description provided for @writingTheSummaryOf.
  ///
  /// In en, this message translates to:
  /// **'Writing the summary of {title}'**
  String writingTheSummaryOf(Object title);

  /// No description provided for @summaryOf.
  ///
  /// In en, this message translates to:
  /// **'Summary of {title}'**
  String summaryOf(Object title);

  /// No description provided for @summarizeWith.
  ///
  /// In en, this message translates to:
  /// **'Summarize with {label}'**
  String summarizeWith(Object label);

  /// No description provided for @defaultLabel2.
  ///
  /// In en, this message translates to:
  /// **'{model} · default'**
  String defaultLabel2(Object model);

  /// No description provided for @summarizeOnMyComputer.
  ///
  /// In en, this message translates to:
  /// **'Summarize on my computer'**
  String get summarizeOnMyComputer;

  /// No description provided for @withClaudeCodeCodexOrThe.
  ///
  /// In en, this message translates to:
  /// **'With Claude Code, Codex or the connection set up there'**
  String get withClaudeCodeCodexOrThe;

  /// No description provided for @useYourOwnAiApp2.
  ///
  /// In en, this message translates to:
  /// **'Use your own AI app…'**
  String get useYourOwnAiApp2;

  /// No description provided for @claudeChatgptOrAnotherAppWith.
  ///
  /// In en, this message translates to:
  /// **'Claude, ChatGPT or another app, with your account'**
  String get claudeChatgptOrAnotherAppWith;

  /// No description provided for @openSummary.
  ///
  /// In en, this message translates to:
  /// **'Open summary'**
  String get openSummary;

  /// No description provided for @editPromptForThisMeeting.
  ///
  /// In en, this message translates to:
  /// **'Edit prompt for this meeting…'**
  String get editPromptForThisMeeting;

  /// No description provided for @useTheSettingsPrompt.
  ///
  /// In en, this message translates to:
  /// **'Use the Settings prompt'**
  String get useTheSettingsPrompt;

  /// No description provided for @promptForThisMeeting.
  ///
  /// In en, this message translates to:
  /// **'Prompt for this meeting'**
  String get promptForThisMeeting;

  /// No description provided for @placeholders.
  ///
  /// In en, this message translates to:
  /// **'Placeholders: {placeholders}'**
  String placeholders(Object placeholders);

  /// No description provided for @addAnAiConnectionInSettings3.
  ///
  /// In en, this message translates to:
  /// **'Add an AI connection in Settings to write summaries.'**
  String get addAnAiConnectionInSettings3;

  /// No description provided for @summarizeWith2.
  ///
  /// In en, this message translates to:
  /// **'Summarize with'**
  String get summarizeWith2;

  /// No description provided for @summarize.
  ///
  /// In en, this message translates to:
  /// **'Summarize'**
  String get summarize;

  /// No description provided for @summarizeAgain.
  ///
  /// In en, this message translates to:
  /// **'Summarize again'**
  String get summarizeAgain;

  /// No description provided for @editPromptForThisMeeting2.
  ///
  /// In en, this message translates to:
  /// **'Edit prompt for this meeting'**
  String get editPromptForThisMeeting2;

  /// No description provided for @writingTheSummaryALocalModel.
  ///
  /// In en, this message translates to:
  /// **'Writing the summary… A local model can take a few minutes.'**
  String get writingTheSummaryALocalModel;

  /// No description provided for @noSummaryYet.
  ///
  /// In en, this message translates to:
  /// **'No summary yet.'**
  String get noSummaryYet;

  /// No description provided for @writeASummary.
  ///
  /// In en, this message translates to:
  /// **'Write a summary'**
  String get writeASummary;

  /// No description provided for @writtenByYouOn.
  ///
  /// In en, this message translates to:
  /// **'Written by you on {createdAt}'**
  String writtenByYouOn(Object createdAt);

  /// No description provided for @writtenByOn.
  ///
  /// In en, this message translates to:
  /// **'Written by {model} ({connectionLabel}) on {createdAt}{edited}'**
  String writtenByOn(
      Object model, Object connectionLabel, Object createdAt, Object edited);

  /// No description provided for @editSummary.
  ///
  /// In en, this message translates to:
  /// **'Edit summary'**
  String get editSummary;

  /// No description provided for @tryAgainOnMyComputer.
  ///
  /// In en, this message translates to:
  /// **'Try again on my computer'**
  String get tryAgainOnMyComputer;

  /// No description provided for @cancelRequest.
  ///
  /// In en, this message translates to:
  /// **'Cancel request'**
  String get cancelRequest;

  /// No description provided for @notesFolder.
  ///
  /// In en, this message translates to:
  /// **'Notes folder'**
  String get notesFolder;

  /// No description provided for @keepYourProjectsAndMeetingsAs.
  ///
  /// In en, this message translates to:
  /// **'Keep your projects and meetings as plain files in a folder, to read and edit them with any editor. Edits to notes.md, summary.md and project files come back into the app within half a minute, and files you drop into a project folder or a meeting\'s attachments folder are added. transcript.md is rewritten by the app: correct the transcript in the app.'**
  String get keepYourProjectsAndMeetingsAs;

  /// No description provided for @theFolderIsADecryptedCopy.
  ///
  /// In en, this message translates to:
  /// **'The folder is a decrypted copy of your notes. Anyone who can read it can read them, and backup or sync tools will copy it. Keep it on an encrypted disk, inside your own user folder.'**
  String get theFolderIsADecryptedCopy;

  /// No description provided for @folder.
  ///
  /// In en, this message translates to:
  /// **'Folder'**
  String get folder;

  /// No description provided for @chooseAFolder.
  ///
  /// In en, this message translates to:
  /// **'Choose a folder'**
  String get chooseAFolder;

  /// No description provided for @keepTheNotesFolder.
  ///
  /// In en, this message translates to:
  /// **'Keep the notes folder'**
  String get keepTheNotesFolder;

  /// No description provided for @syncNow.
  ///
  /// In en, this message translates to:
  /// **'Sync now'**
  String get syncNow;

  /// No description provided for @lastSyncAtFailed.
  ///
  /// In en, this message translates to:
  /// **'Last sync at {at} failed: {code}'**
  String lastSyncAtFailed(Object at, Object code);

  /// No description provided for @lastSyncAtFilesWrittenEdits.
  ///
  /// In en, this message translates to:
  /// **'Last sync at {at}: {written} files written, {imported} edits and files taken in.'**
  String lastSyncAtFilesWrittenEdits(
      Object at, Object written, Object imported);

  /// No description provided for @inviteADevice.
  ///
  /// In en, this message translates to:
  /// **'Invite a device'**
  String get inviteADevice;

  /// No description provided for @pairingInviteQrCode.
  ///
  /// In en, this message translates to:
  /// **'Pairing invite QR code'**
  String get pairingInviteQrCode;

  /// No description provided for @onTheOtherDeviceOpenDevices.
  ///
  /// In en, this message translates to:
  /// **'On the other device, open Devices, choose Use invite, and paste this link. You can scan the code with a camera app to copy it. It works until {hour}:{minute}.'**
  String onTheOtherDeviceOpenDevices(Object hour, Object minute);

  /// No description provided for @close.
  ///
  /// In en, this message translates to:
  /// **'Close'**
  String get close;

  /// No description provided for @inviteLinkCopied.
  ///
  /// In en, this message translates to:
  /// **'Invite link copied'**
  String get inviteLinkCopied;

  /// No description provided for @copyLink.
  ///
  /// In en, this message translates to:
  /// **'Copy link'**
  String get copyLink;

  /// No description provided for @deviceFoundChoosePairUnderNearby.
  ///
  /// In en, this message translates to:
  /// **'Device found. Choose Pair under Nearby devices.'**
  String get deviceFoundChoosePairUnderNearby;

  /// No description provided for @useAnInvite.
  ///
  /// In en, this message translates to:
  /// **'Use an invite'**
  String get useAnInvite;

  /// No description provided for @inviteLink.
  ///
  /// In en, this message translates to:
  /// **'Invite link'**
  String get inviteLink;

  /// No description provided for @findDevice.
  ///
  /// In en, this message translates to:
  /// **'Find device'**
  String get findDevice;

  /// No description provided for @pairWithAnotherDevice.
  ///
  /// In en, this message translates to:
  /// **'Pair with another device?'**
  String get pairWithAnotherDevice;

  /// No description provided for @youOpenedAPairingInviteKoalino.
  ///
  /// In en, this message translates to:
  /// **'You opened a pairing invite. Koalino will find the device that shows it, and both devices will show a code to compare before anything is shared.'**
  String get youOpenedAPairingInviteKoalino;

  /// No description provided for @pairingStartedCheckThatBothDevices.
  ///
  /// In en, this message translates to:
  /// **'Pairing started. Check that both devices show the same code.'**
  String get pairingStartedCheckThatBothDevices;

  /// No description provided for @done.
  ///
  /// In en, this message translates to:
  /// **'Done'**
  String get done;

  /// No description provided for @photosAreAddedToTheMeeting.
  ///
  /// In en, this message translates to:
  /// **'Photos are added to the meeting as you take them.'**
  String get photosAreAddedToTheMeeting;

  /// No description provided for @takePhoto.
  ///
  /// In en, this message translates to:
  /// **'Take photo'**
  String get takePhoto;

  /// No description provided for @theCameraIsUnavailable.
  ///
  /// In en, this message translates to:
  /// **'The camera is unavailable.'**
  String get theCameraIsUnavailable;

  /// No description provided for @search.
  ///
  /// In en, this message translates to:
  /// **'Search {name}'**
  String search(Object name);

  /// No description provided for @noteSaved.
  ///
  /// In en, this message translates to:
  /// **'Note saved'**
  String get noteSaved;

  /// No description provided for @theNoteCouldNotBeSaved.
  ///
  /// In en, this message translates to:
  /// **'The note could not be saved.'**
  String get theNoteCouldNotBeSaved;

  /// No description provided for @couldNotBeAdded2.
  ///
  /// In en, this message translates to:
  /// **'{failed} could not be added.'**
  String couldNotBeAdded2(Object failed);

  /// No description provided for @renameProject.
  ///
  /// In en, this message translates to:
  /// **'Rename project'**
  String get renameProject;

  /// No description provided for @theProjectCouldNotBeRenamed.
  ///
  /// In en, this message translates to:
  /// **'The project could not be renamed.'**
  String get theProjectCouldNotBeRenamed;

  /// No description provided for @delete3.
  ///
  /// In en, this message translates to:
  /// **'Delete \"{project}\"?'**
  String delete3(Object project);

  /// No description provided for @itsFilesNotesAndEMails.
  ///
  /// In en, this message translates to:
  /// **'Its files, notes and e-mails are deleted on every paired device. Its meetings are kept, outside any project.'**
  String get itsFilesNotesAndEMails;

  /// No description provided for @theProjectCouldNotBeDeleted.
  ///
  /// In en, this message translates to:
  /// **'The project could not be deleted.'**
  String get theProjectCouldNotBeDeleted;

  /// No description provided for @itIsDeletedOnEveryPaired.
  ///
  /// In en, this message translates to:
  /// **'It is deleted on every paired device.'**
  String get itIsDeletedOnEveryPaired;

  /// No description provided for @theFileCouldNotBeDeleted.
  ///
  /// In en, this message translates to:
  /// **'The file could not be deleted.'**
  String get theFileCouldNotBeDeleted;

  /// No description provided for @theMeetingCouldNotBeRemoved.
  ///
  /// In en, this message translates to:
  /// **'The meeting could not be removed from the project.'**
  String get theMeetingCouldNotBeRemoved;

  /// No description provided for @project.
  ///
  /// In en, this message translates to:
  /// **'Project'**
  String get project;

  /// No description provided for @searchThisProject.
  ///
  /// In en, this message translates to:
  /// **'Search this project'**
  String get searchThisProject;

  /// No description provided for @chatAboutThisProject.
  ///
  /// In en, this message translates to:
  /// **'Chat about this project'**
  String get chatAboutThisProject;

  /// No description provided for @addFiles.
  ///
  /// In en, this message translates to:
  /// **'Add files'**
  String get addFiles;

  /// No description provided for @writeANote.
  ///
  /// In en, this message translates to:
  /// **'Write a note'**
  String get writeANote;

  /// No description provided for @addMeetings.
  ///
  /// In en, this message translates to:
  /// **'Add meetings'**
  String get addMeetings;

  /// No description provided for @deleteProject.
  ///
  /// In en, this message translates to:
  /// **'Delete project'**
  String get deleteProject;

  /// No description provided for @addThemHereOrFromA.
  ///
  /// In en, this message translates to:
  /// **'Add them here, or from a meeting\'s page.'**
  String get addThemHereOrFromA;

  /// No description provided for @removeFromProject.
  ///
  /// In en, this message translates to:
  /// **'Remove from project'**
  String get removeFromProject;

  /// No description provided for @filesNotesAndEMails.
  ///
  /// In en, this message translates to:
  /// **'Files, notes and e-mails'**
  String get filesNotesAndEMails;

  /// No description provided for @nothingAddedYet.
  ///
  /// In en, this message translates to:
  /// **'Nothing added yet'**
  String get nothingAddedYet;

  /// No description provided for @addPicturesMarkdownPdfsEMails.
  ///
  /// In en, this message translates to:
  /// **'Add pictures, Markdown, PDFs, e-mails (.eml) or documents.'**
  String get addPicturesMarkdownPdfsEMails;

  /// No description provided for @onItsWayFromAPaired.
  ///
  /// In en, this message translates to:
  /// **'On its way from a paired device'**
  String get onItsWayFromAPaired;

  /// No description provided for @newNote.
  ///
  /// In en, this message translates to:
  /// **'New note'**
  String get newNote;

  /// No description provided for @noteMarkdown.
  ///
  /// In en, this message translates to:
  /// **'Note (Markdown)'**
  String get noteMarkdown;

  /// No description provided for @everyMeetingIsAlreadyInA.
  ///
  /// In en, this message translates to:
  /// **'Every meeting is already in a project.'**
  String get everyMeetingIsAlreadyInA;

  /// No description provided for @add.
  ///
  /// In en, this message translates to:
  /// **'Add'**
  String get add;

  /// No description provided for @moveToProject.
  ///
  /// In en, this message translates to:
  /// **'Move to project'**
  String get moveToProject;

  /// No description provided for @noProject.
  ///
  /// In en, this message translates to:
  /// **'No project'**
  String get noProject;

  /// No description provided for @newProject.
  ///
  /// In en, this message translates to:
  /// **'New project…'**
  String get newProject;

  /// No description provided for @newProject2.
  ///
  /// In en, this message translates to:
  /// **'New project'**
  String get newProject2;

  /// No description provided for @create.
  ///
  /// In en, this message translates to:
  /// **'Create'**
  String get create;

  /// No description provided for @theProjectCouldNotBeCreated.
  ///
  /// In en, this message translates to:
  /// **'The project could not be created.'**
  String get theProjectCouldNotBeCreated;

  /// No description provided for @theMeetingCouldNotBeMoved.
  ///
  /// In en, this message translates to:
  /// **'The meeting could not be moved.'**
  String get theMeetingCouldNotBeMoved;

  /// No description provided for @removedFromItsProject.
  ///
  /// In en, this message translates to:
  /// **'Removed from its project'**
  String get removedFromItsProject;

  /// No description provided for @movedTo.
  ///
  /// In en, this message translates to:
  /// **'Moved to {project}'**
  String movedTo(Object project);

  /// No description provided for @searchProjects.
  ///
  /// In en, this message translates to:
  /// **'Search projects'**
  String get searchProjects;

  /// No description provided for @noProjectsYetAProjectGathers.
  ///
  /// In en, this message translates to:
  /// **'No projects yet. A project gathers meetings with the files, notes and e-mails that belong to them.'**
  String get noProjectsYetAProjectGathers;

  /// No description provided for @theRecordingIsStillStartingTry.
  ///
  /// In en, this message translates to:
  /// **'The recording is still starting; try again.'**
  String get theRecordingIsStillStartingTry;

  /// No description provided for @recording.
  ///
  /// In en, this message translates to:
  /// **'Recording \"{title}\"'**
  String recording(Object title);

  /// No description provided for @recording2.
  ///
  /// In en, this message translates to:
  /// **'Recording'**
  String get recording2;

  /// No description provided for @paused.
  ///
  /// In en, this message translates to:
  /// **'Paused \"{title}\"'**
  String paused(Object title);

  /// No description provided for @paused2.
  ///
  /// In en, this message translates to:
  /// **'Paused'**
  String get paused2;

  /// No description provided for @finishingTranscription.
  ///
  /// In en, this message translates to:
  /// **'Finishing transcription…'**
  String get finishingTranscription;

  /// No description provided for @readyToRecord.
  ///
  /// In en, this message translates to:
  /// **'Ready to record'**
  String get readyToRecord;

  /// No description provided for @meetingTitle.
  ///
  /// In en, this message translates to:
  /// **'Meeting title'**
  String get meetingTitle;

  /// No description provided for @enterMeetingTitleOptional.
  ///
  /// In en, this message translates to:
  /// **'Enter meeting title (optional)'**
  String get enterMeetingTitleOptional;

  /// No description provided for @leaveEmptyToNameTheMeeting.
  ///
  /// In en, this message translates to:
  /// **'Leave empty to name the meeting by its start time'**
  String get leaveEmptyToNameTheMeeting;

  /// No description provided for @startingRecording.
  ///
  /// In en, this message translates to:
  /// **'Starting recording'**
  String get startingRecording;

  /// No description provided for @startRecording.
  ///
  /// In en, this message translates to:
  /// **'Start recording'**
  String get startRecording;

  /// No description provided for @pause.
  ///
  /// In en, this message translates to:
  /// **'Pause'**
  String get pause;

  /// No description provided for @resume.
  ///
  /// In en, this message translates to:
  /// **'Resume'**
  String get resume;

  /// No description provided for @stop.
  ///
  /// In en, this message translates to:
  /// **'Stop'**
  String get stop;

  /// No description provided for @language.
  ///
  /// In en, this message translates to:
  /// **'Language'**
  String get language;

  /// No description provided for @aChangeAppliesToWhatIs.
  ///
  /// In en, this message translates to:
  /// **'A change applies to what is said next'**
  String get aChangeAppliesToWhatIs;

  /// No description provided for @liveTranscript.
  ///
  /// In en, this message translates to:
  /// **'Live transcript'**
  String get liveTranscript;

  /// No description provided for @theTranscriptAppearsHereAFew.
  ///
  /// In en, this message translates to:
  /// **'The transcript appears here a few seconds after each pause in speech.'**
  String get theTranscriptAppearsHereAFew;

  /// No description provided for @nearbyDevicesSeeThisNameWhile.
  ///
  /// In en, this message translates to:
  /// **'Nearby devices see this name while this device is discoverable. Devices already paired keep the name they learned.'**
  String get nearbyDevicesSeeThisNameWhile;

  /// No description provided for @meetingRenamed.
  ///
  /// In en, this message translates to:
  /// **'Meeting renamed'**
  String get meetingRenamed;

  /// No description provided for @thisTitleIsnTAllowed.
  ///
  /// In en, this message translates to:
  /// **'This title isn\'t allowed.'**
  String get thisTitleIsnTAllowed;

  /// No description provided for @couldnTSaveTheNewTitle.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t save the new title. Try again.'**
  String get couldnTSaveTheNewTitle;

  /// No description provided for @theNoteTakingEngineIsnT.
  ///
  /// In en, this message translates to:
  /// **'The note-taking engine isn\'t available right now.'**
  String get theNoteTakingEngineIsnT;

  /// No description provided for @thisMeetingNoLongerExists2.
  ///
  /// In en, this message translates to:
  /// **'This meeting no longer exists.'**
  String get thisMeetingNoLongerExists2;

  /// No description provided for @couldnTRenameTheMeeting.
  ///
  /// In en, this message translates to:
  /// **'Couldn\'t rename the meeting.'**
  String get couldnTRenameTheMeeting;

  /// No description provided for @thisFileIsStillOnIts.
  ///
  /// In en, this message translates to:
  /// **'This file is still on its way from a paired device.'**
  String get thisFileIsStillOnIts;

  /// No description provided for @thisPictureCannotBeShown.
  ///
  /// In en, this message translates to:
  /// **'This picture cannot be shown.'**
  String get thisPictureCannotBeShown;

  /// No description provided for @thisKindOfDocumentCanT.
  ///
  /// In en, this message translates to:
  /// **'This kind of document can\'t be shown in the app yet.'**
  String get thisKindOfDocumentCanT;

  /// No description provided for @thisFileCannotBeOpened.
  ///
  /// In en, this message translates to:
  /// **'This file cannot be opened.'**
  String get thisFileCannotBeOpened;

  /// No description provided for @thisFileCannotBeRead.
  ///
  /// In en, this message translates to:
  /// **'This file cannot be read.'**
  String get thisFileCannotBeRead;

  /// No description provided for @thisEMailCannotBeRead.
  ///
  /// In en, this message translates to:
  /// **'This e-mail cannot be read.'**
  String get thisEMailCannotBeRead;

  /// No description provided for @noSubject.
  ///
  /// In en, this message translates to:
  /// **'(no subject)'**
  String get noSubject;

  /// No description provided for @from.
  ///
  /// In en, this message translates to:
  /// **'From'**
  String get from;

  /// No description provided for @to.
  ///
  /// In en, this message translates to:
  /// **'To'**
  String get to;

  /// No description provided for @cc.
  ///
  /// In en, this message translates to:
  /// **'Cc'**
  String get cc;

  /// No description provided for @sent.
  ///
  /// In en, this message translates to:
  /// **'Sent'**
  String get sent;

  /// No description provided for @attachments.
  ///
  /// In en, this message translates to:
  /// **'Attachments: {attachmentNames}'**
  String attachments(Object attachmentNames);

  /// No description provided for @theRestOfThisEMail.
  ///
  /// In en, this message translates to:
  /// **'The rest of this e-mail is too long to show.'**
  String get theRestOfThisEMail;

  /// No description provided for @searching.
  ///
  /// In en, this message translates to:
  /// **'Searching…'**
  String get searching;

  /// No description provided for @nothingFound.
  ///
  /// In en, this message translates to:
  /// **'Nothing found.'**
  String get nothingFound;

  /// No description provided for @search2.
  ///
  /// In en, this message translates to:
  /// **'Search'**
  String get search2;

  /// No description provided for @pairYourDevicesAndSeeWhich.
  ///
  /// In en, this message translates to:
  /// **'Pair your devices and see which ones are connected.'**
  String get pairYourDevicesAndSeeWhich;

  /// No description provided for @transcriptionModel.
  ///
  /// In en, this message translates to:
  /// **'Transcription model'**
  String get transcriptionModel;

  /// No description provided for @chooseTheWhisperModel.
  ///
  /// In en, this message translates to:
  /// **'Choose the Whisper model.'**
  String get chooseTheWhisperModel;

  /// No description provided for @languageModelsForMeetingSummariesAnd.
  ///
  /// In en, this message translates to:
  /// **'Language models for meeting summaries, and the summary prompt.'**
  String get languageModelsForMeetingSummariesAnd;

  /// No description provided for @keepProjectsAndMeetingsAsFiles.
  ///
  /// In en, this message translates to:
  /// **'Keep projects and meetings as files you can edit with any editor.'**
  String get keepProjectsAndMeetingsAsFiles;

  /// No description provided for @letYourOwnAssistantSuchAs.
  ///
  /// In en, this message translates to:
  /// **'Let your own assistant, such as Claude Code, read your notes.'**
  String get letYourOwnAssistantSuchAs;

  /// No description provided for @appLock.
  ///
  /// In en, this message translates to:
  /// **'App lock'**
  String get appLock;

  /// No description provided for @defaultLanguage.
  ///
  /// In en, this message translates to:
  /// **'Default language'**
  String get defaultLanguage;

  /// No description provided for @usedForNewRecordingsAutoDetects.
  ///
  /// In en, this message translates to:
  /// **'Used for new recordings. Auto detects the language as people speak.'**
  String get usedForNewRecordingsAutoDetects;

  /// No description provided for @transcribeOn.
  ///
  /// In en, this message translates to:
  /// **'Transcribe on'**
  String get transcribeOn;

  /// No description provided for @anotherDeviceReceivesTheRecordingS.
  ///
  /// In en, this message translates to:
  /// **'Another device receives the recording\'s audio, encrypted, and deletes it once transcribed. If it is unavailable, this device transcribes instead.'**
  String get anotherDeviceReceivesTheRecordingS;

  /// No description provided for @telemetry.
  ///
  /// In en, this message translates to:
  /// **'Telemetry'**
  String get telemetry;

  /// No description provided for @howLongTranscriptionTakesOnEach.
  ///
  /// In en, this message translates to:
  /// **'How long transcription takes on each device, and its estimated energy use.'**
  String get howLongTranscriptionTakesOnEach;

  /// No description provided for @logLevel.
  ///
  /// In en, this message translates to:
  /// **'Log level'**
  String get logLevel;

  /// No description provided for @howMuchDetailTheAppWrites.
  ///
  /// In en, this message translates to:
  /// **'How much detail the app writes to its logs. Debug and Trace help diagnose problems but write more.'**
  String get howMuchDetailTheAppWrites;

  /// No description provided for @custom.
  ///
  /// In en, this message translates to:
  /// **'Custom'**
  String get custom;

  /// No description provided for @shareLogs.
  ///
  /// In en, this message translates to:
  /// **'Share logs'**
  String get shareLogs;

  /// No description provided for @theLast7DaysOfDiagnostic.
  ///
  /// In en, this message translates to:
  /// **'The last 7 days of diagnostic logs, for example to attach to a bug report.'**
  String get theLast7DaysOfDiagnostic;

  /// No description provided for @unableToReadTheAppLock.
  ///
  /// In en, this message translates to:
  /// **'Unable to read the app lock setting.'**
  String get unableToReadTheAppLock;

  /// No description provided for @needsAndroid10OrLater.
  ///
  /// In en, this message translates to:
  /// **'Needs Android 10 or later.'**
  String get needsAndroid10OrLater;

  /// No description provided for @setAScreenLockInAndroid.
  ///
  /// In en, this message translates to:
  /// **'Set a screen lock in Android settings to use App lock.'**
  String get setAScreenLockInAndroid;

  /// No description provided for @askForYourFingerprintFaceOr.
  ///
  /// In en, this message translates to:
  /// **'Ask for your fingerprint, face, or screen lock when you return to Koalino after {lockAfterBackground} seconds away. Recording and sync keep running while it is locked.'**
  String askForYourFingerprintFaceOr(Object lockAfterBackground);

  /// No description provided for @otherNetworks.
  ///
  /// In en, this message translates to:
  /// **'Other networks'**
  String get otherNetworks;

  /// No description provided for @aRelayOnYourOwnAlways.
  ///
  /// In en, this message translates to:
  /// **'A relay on your own always-on device lets your devices reach each other away from the same Wi-Fi. Syncing on the same network keeps working without it.'**
  String get aRelayOnYourOwnAlways;

  /// No description provided for @relayAddress.
  ///
  /// In en, this message translates to:
  /// **'Relay address'**
  String get relayAddress;

  /// No description provided for @deviceAddressesOptionalOnePerLine.
  ///
  /// In en, this message translates to:
  /// **'Device addresses (optional, one per line)'**
  String get deviceAddressesOptionalOnePerLine;

  /// No description provided for @saveAddresses.
  ///
  /// In en, this message translates to:
  /// **'Save addresses'**
  String get saveAddresses;

  /// No description provided for @nothingThatWasSharedCouldBe.
  ///
  /// In en, this message translates to:
  /// **'Nothing that was shared could be read.'**
  String get nothingThatWasSharedCouldBe;

  /// No description provided for @couldNotBeRead.
  ///
  /// In en, this message translates to:
  /// **'{message}; {failedCount} could not be read'**
  String couldNotBeRead(Object message, Object failedCount);

  /// No description provided for @saveAsTheSummaryOf.
  ///
  /// In en, this message translates to:
  /// **'Save as the summary of \"{title}\"'**
  String saveAsTheSummaryOf(Object title);

  /// No description provided for @addToAProjectInstead.
  ///
  /// In en, this message translates to:
  /// **'Add to a project instead'**
  String get addToAProjectInstead;

  /// No description provided for @splitThisMeeting.
  ///
  /// In en, this message translates to:
  /// **'Split this meeting?'**
  String get splitThisMeeting;

  /// No description provided for @itWasRecordedOnAndThe.
  ///
  /// In en, this message translates to:
  /// **'It was recorded on {devices}, and the app joined the recordings. Splitting lists the other {recordings} on its own again, with its own transcript, here and on your paired devices. They are not joined again. This meeting keeps its transcript.'**
  String itWasRecordedOnAndThe(Object devices, Object recordings);

  /// No description provided for @unableToSplitThisMeeting.
  ///
  /// In en, this message translates to:
  /// **'Unable to split this meeting.'**
  String get unableToSplitThisMeeting;

  /// No description provided for @telemetryDashboard.
  ///
  /// In en, this message translates to:
  /// **'Telemetry Dashboard'**
  String get telemetryDashboard;

  /// No description provided for @transcription.
  ///
  /// In en, this message translates to:
  /// **'Transcription'**
  String get transcription;

  /// No description provided for @latency.
  ///
  /// In en, this message translates to:
  /// **'Latency'**
  String get latency;

  /// No description provided for @tasks.
  ///
  /// In en, this message translates to:
  /// **'Tasks'**
  String get tasks;

  /// No description provided for @power.
  ///
  /// In en, this message translates to:
  /// **'Power'**
  String get power;

  /// No description provided for @transcriptionDurationOverTime.
  ///
  /// In en, this message translates to:
  /// **'Transcription Duration Over Time'**
  String get transcriptionDurationOverTime;

  /// No description provided for @latencyDistribution.
  ///
  /// In en, this message translates to:
  /// **'Latency Distribution'**
  String get latencyDistribution;

  /// No description provided for @taskTypeBreakdown.
  ///
  /// In en, this message translates to:
  /// **'Task Type Breakdown'**
  String get taskTypeBreakdown;

  /// No description provided for @powerConsumptionEstimate.
  ///
  /// In en, this message translates to:
  /// **'Power Consumption Estimate'**
  String get powerConsumptionEstimate;

  /// No description provided for @fastestLeastAccurate.
  ///
  /// In en, this message translates to:
  /// **'Fastest, least accurate'**
  String get fastestLeastAccurate;

  /// No description provided for @fastManyMistakesTheAppShips.
  ///
  /// In en, this message translates to:
  /// **'Fast, many mistakes. The app ships with this one'**
  String get fastManyMistakesTheAppShips;

  /// No description provided for @goodBalanceOnMostComputers.
  ///
  /// In en, this message translates to:
  /// **'Good balance on most computers'**
  String get goodBalanceOnMostComputers;

  /// No description provided for @accurateSlowerNeedsAbout2Gb.
  ///
  /// In en, this message translates to:
  /// **'Accurate, slower, needs about 2 GB of memory'**
  String get accurateSlowerNeedsAbout2Gb;

  /// No description provided for @olderNeedsAPowerfulComputer.
  ///
  /// In en, this message translates to:
  /// **'Older, needs a powerful computer'**
  String get olderNeedsAPowerfulComputer;

  /// No description provided for @mostAccurateNeedsAPowerfulComputer.
  ///
  /// In en, this message translates to:
  /// **'Most accurate, needs a powerful computer'**
  String get mostAccurateNeedsAPowerfulComputer;

  /// No description provided for @veryAccurateBestWithAGraphics.
  ///
  /// In en, this message translates to:
  /// **'Very accurate. Best with a graphics card'**
  String get veryAccurateBestWithAGraphics;

  /// No description provided for @thisModelMayBeTooLarge.
  ///
  /// In en, this message translates to:
  /// **'This model may be too large'**
  String get thisModelMayBeTooLarge;

  /// No description provided for @mayNeedMoreMemoryThanThis.
  ///
  /// In en, this message translates to:
  /// **'{model} may need more memory than this device has, and recordings could fail or run slowly.'**
  String mayNeedMoreMemoryThanThis(Object model);

  /// No description provided for @downloadAnyway.
  ///
  /// In en, this message translates to:
  /// **'Download anyway'**
  String get downloadAnyway;

  /// No description provided for @transcriptionSettingsAreUnavailable.
  ///
  /// In en, this message translates to:
  /// **'Transcription settings are unavailable.'**
  String get transcriptionSettingsAreUnavailable;

  /// No description provided for @downloadFromQualcomm.
  ///
  /// In en, this message translates to:
  /// **'Download from Qualcomm?'**
  String get downloadFromQualcomm;

  /// No description provided for @about360MbIsDownloadedFrom.
  ///
  /// In en, this message translates to:
  /// **'About 360 MB is downloaded from Qualcomm and needs about 460 MB of space:\n\n• Qualcomm\'s QNN runtime, which lets apps use the NPU. It is licensed under Qualcomm\'s AI Stack License, not the app\'s licence.\n• Whisper Small, compiled by Qualcomm AI Hub for this phone\'s NPU.\n\nEach file is checked before it is used. Your recordings never leave the phone. By downloading, you accept Qualcomm\'s terms.'**
  String get about360MbIsDownloadedFrom;

  /// No description provided for @acceptAndDownload.
  ///
  /// In en, this message translates to:
  /// **'Accept and download'**
  String get acceptAndDownload;

  /// No description provided for @downloadingTheNpuFiles.
  ///
  /// In en, this message translates to:
  /// **'Downloading the NPU files…'**
  String get downloadingTheNpuFiles;

  /// No description provided for @transcribeOnTheNpu.
  ///
  /// In en, this message translates to:
  /// **'Transcribe on the NPU'**
  String get transcribeOnTheNpu;

  /// No description provided for @thisPhoneHasAnAiChip.
  ///
  /// In en, this message translates to:
  /// **'This phone has an AI chip (NPU) that can run Whisper Small, leaving the processor free: 30 seconds of speech take about 2 seconds. It needs a download from Qualcomm.'**
  String get thisPhoneHasAnAiChip;

  /// No description provided for @download.
  ///
  /// In en, this message translates to:
  /// **'Download'**
  String get download;

  /// No description provided for @runsWhisperSmallOnThePhone.
  ///
  /// In en, this message translates to:
  /// **'Runs Whisper Small on the phone\'s AI chip, in place of the model chosen below, and for the after-meeting pass unless it has its own model. Word times are approximate.'**
  String get runsWhisperSmallOnThePhone;

  /// No description provided for @removeTheNpuFiles.
  ///
  /// In en, this message translates to:
  /// **'Remove the NPU files'**
  String get removeTheNpuFiles;

  /// No description provided for @alsoRecordTheComputerSSound.
  ///
  /// In en, this message translates to:
  /// **'Also record the computer\'s sound'**
  String get alsoRecordTheComputerSSound;

  /// No description provided for @transcribesWhatTheComputerPlaysSuch.
  ///
  /// In en, this message translates to:
  /// **'Transcribes what the computer plays, such as the other side of a call, beside the microphone. When the microphone also hears the speakers, the repeated words are kept once.'**
  String get transcribesWhatTheComputerPlaysSuch;

  /// No description provided for @improveTheTranscriptAfterTheMeeting.
  ///
  /// In en, this message translates to:
  /// **'Improve the transcript after the meeting'**
  String get improveTheTranscriptAfterTheMeeting;

  /// No description provided for @transcribesEachMeetingAgainWithMore.
  ///
  /// In en, this message translates to:
  /// **'Transcribes each meeting again with more context once it ends; a phone waits until it is charging. The speech is kept encrypted until then, then deleted.'**
  String get transcribesEachMeetingAgainWithMore;

  /// No description provided for @modelForThisPass.
  ///
  /// In en, this message translates to:
  /// **'Model for this pass'**
  String get modelForThisPass;

  /// No description provided for @aLargerModelIsSlowerBut.
  ///
  /// In en, this message translates to:
  /// **'A larger model is slower but more accurate, and the pass has time after the meeting. Download models above.'**
  String get aLargerModelIsSlowerBut;

  /// No description provided for @sameAsForRecording.
  ///
  /// In en, this message translates to:
  /// **'Same as for recording'**
  String get sameAsForRecording;

  /// No description provided for @keepSpeechForCombining.
  ///
  /// In en, this message translates to:
  /// **'Keep speech for combining'**
  String get keepSpeechForCombining;

  /// No description provided for @whenAMeetingWasAlsoRecorded.
  ///
  /// In en, this message translates to:
  /// **'When a meeting was also recorded on another device, its speech waits this long to be combined, then is deleted.'**
  String get whenAMeetingWasAlsoRecorded;

  /// No description provided for @whisperModel.
  ///
  /// In en, this message translates to:
  /// **'Whisper model'**
  String get whisperModel;

  /// No description provided for @biggerModelsMakeFewerMistakesAnd.
  ///
  /// In en, this message translates to:
  /// **'Bigger models make fewer mistakes, and need more time and memory. Downloads come from Hugging Face. This choice applies only to this device.'**
  String get biggerModelsMakeFewerMistakesAnd;

  /// No description provided for @automatic2.
  ///
  /// In en, this message translates to:
  /// **'Automatic: {fileName}{mode}'**
  String automatic2(Object fileName, Object mode);

  /// No description provided for @usesUntilIsDownloadedTheBest.
  ///
  /// In en, this message translates to:
  /// **'Uses {fileName} until {waiting} is downloaded, the best fit for this device. {reason}'**
  String usesUntilIsDownloadedTheBest(
      Object fileName, Object waiting, Object reason);

  /// No description provided for @picksTheBestModelForThis.
  ///
  /// In en, this message translates to:
  /// **'Picks the best model for this device: {fileName}{mode}.'**
  String picksTheBestModelForThis(Object fileName, Object mode);

  /// No description provided for @download2.
  ///
  /// In en, this message translates to:
  /// **'Download {sizeBytes}'**
  String download2(Object sizeBytes);

  /// No description provided for @tryLabel.
  ///
  /// In en, this message translates to:
  /// **'Try {model}'**
  String tryLabel(Object model);

  /// No description provided for @use.
  ///
  /// In en, this message translates to:
  /// **'Use'**
  String get use;

  /// No description provided for @suggestedForThisDevice.
  ///
  /// In en, this message translates to:
  /// **'Suggested for this device: {model}{mode}'**
  String suggestedForThisDevice(Object model, Object mode);

  /// No description provided for @efficientMode.
  ///
  /// In en, this message translates to:
  /// **'Efficient mode'**
  String get efficientMode;

  /// No description provided for @runsTheSameModelsOnThe.
  ///
  /// In en, this message translates to:
  /// **'Runs the same models on the graphics chip ({gpuName}), usually faster. Off uses the processor, as standard.'**
  String runsTheSameModelsOnThe(Object gpuName);

  /// No description provided for @theGraphicsChipStoppedTheApp.
  ///
  /// In en, this message translates to:
  /// **'The graphics chip stopped the app last time, so this device is transcribing with its processor.'**
  String get theGraphicsChipStoppedTheApp;

  /// No description provided for @tryTheGraphicsChipAgain.
  ///
  /// In en, this message translates to:
  /// **'Try the graphics chip again'**
  String get tryTheGraphicsChipAgain;

  /// No description provided for @onThisDevice.
  ///
  /// In en, this message translates to:
  /// **'on this device'**
  String get onThisDevice;

  /// No description provided for @usedByAutomatic.
  ///
  /// In en, this message translates to:
  /// **'used by Automatic'**
  String get usedByAutomatic;

  /// No description provided for @englishOnly.
  ///
  /// In en, this message translates to:
  /// **'English only'**
  String get englishOnly;

  /// No description provided for @mayBeTooLargeForThis.
  ///
  /// In en, this message translates to:
  /// **'may be too large for this device'**
  String get mayBeTooLargeForThis;

  /// No description provided for @doesnTMatchYourLanguage.
  ///
  /// In en, this message translates to:
  /// **'doesn\'t match your language'**
  String get doesnTMatchYourLanguage;

  /// No description provided for @notEnoughFreeSpace.
  ///
  /// In en, this message translates to:
  /// **'not enough free space'**
  String get notEnoughFreeSpace;

  /// No description provided for @cancelDownload.
  ///
  /// In en, this message translates to:
  /// **'Cancel download'**
  String get cancelDownload;

  /// No description provided for @glossary.
  ///
  /// In en, this message translates to:
  /// **'Glossary'**
  String get glossary;

  /// No description provided for @namesProjectNamesAndTermsOne.
  ///
  /// In en, this message translates to:
  /// **'Names, project names and terms, one per line. Transcription spells them as written here.'**
  String get namesProjectNamesAndTermsOne;

  /// No description provided for @saveGlossary.
  ///
  /// In en, this message translates to:
  /// **'Save glossary'**
  String get saveGlossary;

  /// No description provided for @voiceGallery.
  ///
  /// In en, this message translates to:
  /// **'Voice Gallery'**
  String get voiceGallery;

  /// No description provided for @tryAgain.
  ///
  /// In en, this message translates to:
  /// **'Try again'**
  String get tryAgain;

  /// No description provided for @unknownVoices.
  ///
  /// In en, this message translates to:
  /// **'Unknown voices'**
  String get unknownVoices;

  /// No description provided for @noUnknownSpeakersToReview.
  ///
  /// In en, this message translates to:
  /// **'No unknown speakers to review'**
  String get noUnknownSpeakersToReview;

  /// No description provided for @savedPeople.
  ///
  /// In en, this message translates to:
  /// **'Saved people'**
  String get savedPeople;

  /// No description provided for @noSavedPeopleYetNameA.
  ///
  /// In en, this message translates to:
  /// **'No saved people yet. Name a voice to add someone.'**
  String get noSavedPeopleYetNameA;

  /// No description provided for @delete4.
  ///
  /// In en, this message translates to:
  /// **'Delete {displayName}'**
  String delete4(Object displayName);

  /// No description provided for @playing.
  ///
  /// In en, this message translates to:
  /// **'Playing…'**
  String get playing;

  /// No description provided for @untitledMeeting.
  ///
  /// In en, this message translates to:
  /// **'Untitled meeting'**
  String get untitledMeeting;

  /// No description provided for @play.
  ///
  /// In en, this message translates to:
  /// **'Play'**
  String get play;

  /// No description provided for @deleteThisVoice.
  ///
  /// In en, this message translates to:
  /// **'Delete this voice'**
  String get deleteThisVoice;

  /// No description provided for @theCallShowedSpeakingCheckThen.
  ///
  /// In en, this message translates to:
  /// **'The call showed {suggestedName} speaking. Check, then Assign.'**
  String theCallShowedSpeakingCheckThen(Object suggestedName);

  /// No description provided for @chooseASavedPerson.
  ///
  /// In en, this message translates to:
  /// **'Choose a saved person'**
  String get chooseASavedPerson;

  /// No description provided for @assign.
  ///
  /// In en, this message translates to:
  /// **'Assign'**
  String get assign;

  /// No description provided for @deleteTheseVoices.
  ///
  /// In en, this message translates to:
  /// **'Delete these voices'**
  String get deleteTheseVoices;

  /// No description provided for @theseSoundLikeOnePersonUntick.
  ///
  /// In en, this message translates to:
  /// **'These sound like one person. Untick any that is someone else.'**
  String get theseSoundLikeOnePersonUntick;

  /// No description provided for @itsVoiceSamplesGoForGood.
  ///
  /// In en, this message translates to:
  /// **'Its {voices} voice samples go for good, here and on your paired devices. The transcripts stay.'**
  String itsVoiceSamplesGoForGood(Object voices);

  /// No description provided for @itsVoiceSampleGoesForGood.
  ///
  /// In en, this message translates to:
  /// **'Its voice sample goes for good, here and on your paired devices. The transcript stays.'**
  String get itsVoiceSampleGoesForGood;

  /// No description provided for @theirNameAndVoiceSamplesGo.
  ///
  /// In en, this message translates to:
  /// **'Their name and voice samples go for good, here and on your paired devices. Transcripts stay and number them as a speaker.'**
  String get theirNameAndVoiceSamplesGo;

  /// No description provided for @unknownSpeaker.
  ///
  /// In en, this message translates to:
  /// **'Unknown speaker {index}'**
  String unknownSpeaker(Object index);

  /// No description provided for @attachedFiles.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Attached 1 file} other{Attached {count} files}}'**
  String attachedFiles(int count);

  /// No description provided for @addedFiles.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Added 1 file} other{Added {count} files}}'**
  String addedFiles(int count);

  /// No description provided for @emailAttachmentsLeft.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 e-mail attachment was left in the e-mail} other{{count} e-mail attachments were left in the e-mail}}'**
  String emailAttachmentsLeft(int count);

  /// No description provided for @addedMeetings.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Added 1 meeting} other{Added {count} meetings}}'**
  String addedMeetings(int count);

  /// Fills {recordings} in itWasRecordedOnAndThe.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{recording} other{{count} recordings}}'**
  String otherRecordings(int count);

  /// No description provided for @severalDevices.
  ///
  /// In en, this message translates to:
  /// **'several devices'**
  String get severalDevices;

  /// No description provided for @meetingCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 meeting} other{{count} meetings}}'**
  String meetingCount(int count);

  /// No description provided for @fileCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 file} other{{count} files}}'**
  String fileCount(int count);

  /// No description provided for @chunkCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 chunk} other{{count} chunks}}'**
  String chunkCount(int count);

  /// No description provided for @resultCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 result} other{{count} results}}'**
  String resultCount(int count);

  /// No description provided for @moreNotShown.
  ///
  /// In en, this message translates to:
  /// **', more not shown: add words to narrow it'**
  String get moreNotShown;

  /// No description provided for @cpuCores.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 CPU core} other{{count} CPU cores}}'**
  String cpuCores(int count);

  /// No description provided for @dayCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 day} other{{count} days}}'**
  String dayCount(int count);

  /// No description provided for @voiceSampleCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =0{no voice sample yet, so not recognized by voice} =1{1 voice sample} other{{count} voice samples}}'**
  String voiceSampleCount(int count);

  /// No description provided for @picturesIncluded.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 picture was included} other{{count} pictures were included}}'**
  String picturesIncluded(int count);

  /// No description provided for @partCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 part} other{{count} parts}}'**
  String partCount(int count);

  /// {meetings} is meetingCount.
  ///
  /// In en, this message translates to:
  /// **'{parts} in {meetings}'**
  String inMeetings(Object parts, Object meetings);

  /// No description provided for @assignedVoicesTo.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Assigned to {name}} other{Assigned {count} voices to {name}}}'**
  String assignedVoicesTo(int count, Object name);

  /// No description provided for @addSharedFiles.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Add the shared file to a project} other{Add {count} shared files to a project}}'**
  String addSharedFiles(int count);

  /// No description provided for @keepSpeechSaved.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{Saved. Speech now waits up to 1 day to be combined.} other{Saved. Speech now waits up to {count} days to be combined.}}'**
  String keepSpeechSaved(int count);

  /// No description provided for @titlesCanBeAtMost.
  ///
  /// In en, this message translates to:
  /// **'Titles can be at most {max} characters.'**
  String titlesCanBeAtMost(Object max);

  /// {example} is the literal text {{placeholder}}.
  ///
  /// In en, this message translates to:
  /// **'The prompt has an unknown or unclosed {example}.'**
  String promptUnknownPlaceholder(Object example);

  /// No description provided for @live.
  ///
  /// In en, this message translates to:
  /// **'● Live'**
  String get live;

  /// No description provided for @offline.
  ///
  /// In en, this message translates to:
  /// **'○ Offline'**
  String get offline;

  /// No description provided for @minutesShort.
  ///
  /// In en, this message translates to:
  /// **'{minutes} min'**
  String minutesShort(Object minutes);

  /// No description provided for @appLanguage.
  ///
  /// In en, this message translates to:
  /// **'App language'**
  String get appLanguage;

  /// No description provided for @appLanguageDescription.
  ///
  /// In en, this message translates to:
  /// **'The language of Koalino\'s menus and messages.'**
  String get appLanguageDescription;

  /// No description provided for @systemLanguage.
  ///
  /// In en, this message translates to:
  /// **'System default'**
  String get systemLanguage;

  /// No description provided for @photosAddedCount.
  ///
  /// In en, this message translates to:
  /// **'{count, plural, =1{1 photo added} other{{count} photos added}}'**
  String photosAddedCount(int count);

  /// No description provided for @moreModels.
  ///
  /// In en, this message translates to:
  /// **'More models ({count})'**
  String moreModels(int count);

  /// No description provided for @moreModelsHint.
  ///
  /// In en, this message translates to:
  /// **'Smaller versions that make a few more mistakes, English-only versions, and models that may not fit this device.'**
  String get moreModelsHint;

  /// No description provided for @microphoneSilencedByCall.
  ///
  /// In en, this message translates to:
  /// **'Android is silencing the microphone while another app uses it for a call: nothing is recorded until the call ends.'**
  String get microphoneSilencedByCall;

  /// No description provided for @handoffInTheBrowser.
  ///
  /// In en, this message translates to:
  /// **'Koalino opens the website in your browser, where you are signed in with your own account, and gives it the meeting with your prompt. The next window shows what to do there.'**
  String get handoffInTheBrowser;

  /// No description provided for @toKeepTheSummaryCopyTheWebsite.
  ///
  /// In en, this message translates to:
  /// **'To keep the summary, copy {appName}\'s answer on the website and come back to Koalino.'**
  String toKeepTheSummaryCopyTheWebsite(String appName);

  /// No description provided for @useYourClaudeOrChatgptPlan.
  ///
  /// In en, this message translates to:
  /// **'Use your Claude or ChatGPT plan'**
  String get useYourClaudeOrChatgptPlan;

  /// No description provided for @summariesCanUseYourOwnPlan.
  ///
  /// In en, this message translates to:
  /// **'Summaries can use your own Claude or ChatGPT subscription, with no API key. Install Claude Code or Codex and sign in once: Koalino then writes summaries by itself.'**
  String get summariesCanUseYourOwnPlan;

  /// No description provided for @installClaudeCode.
  ///
  /// In en, this message translates to:
  /// **'Install Claude Code'**
  String get installClaudeCode;

  /// No description provided for @forAClaudePlanOpensTheGuide.
  ///
  /// In en, this message translates to:
  /// **'For a Claude plan. Opens the install guide.'**
  String get forAClaudePlanOpensTheGuide;

  /// No description provided for @installCodex.
  ///
  /// In en, this message translates to:
  /// **'Install Codex'**
  String get installCodex;

  /// No description provided for @forAChatgptPlanOpensTheGuide.
  ///
  /// In en, this message translates to:
  /// **'For a ChatGPT plan. Opens the install guide.'**
  String get forAChatgptPlanOpensTheGuide;

  /// No description provided for @skipUseTheWebsite.
  ///
  /// In en, this message translates to:
  /// **'Skip: use the website'**
  String get skipUseTheWebsite;

  /// No description provided for @koalinoCopiesTheMeetingAndOpens.
  ///
  /// In en, this message translates to:
  /// **'Nothing to install: Koalino copies the meeting and opens claude.ai or chatgpt.com, and you paste it there.'**
  String get koalinoCopiesTheMeetingAndOpens;

  /// No description provided for @addAConnection.
  ///
  /// In en, this message translates to:
  /// **'Add a connection'**
  String get addAConnection;

  /// No description provided for @claudeCodeOrCodexAlreadyInstalled.
  ///
  /// In en, this message translates to:
  /// **'Claude Code or Codex already installed, or a server on your network.'**
  String get claudeCodeOrCodexAlreadyInstalled;

  /// No description provided for @noConnectionNeededUseTheWebsite.
  ///
  /// In en, this message translates to:
  /// **'No connection needed. In a meeting, open Summary and choose Use your own AI app…: Koalino copies the meeting and opens claude.ai or chatgpt.com.'**
  String get noConnectionNeededUseTheWebsite;

  /// No description provided for @afterInstallingAddTheConnection.
  ///
  /// In en, this message translates to:
  /// **'After installing it and signing in, come back and choose Add connection → Add a connection → Claude Code or Codex.'**
  String get afterInstallingAddTheConnection;

  /// No description provided for @claudeAiOrChatgptComInYourBrowser.
  ///
  /// In en, this message translates to:
  /// **'claude.ai or chatgpt.com in your browser, with your account'**
  String get claudeAiOrChatgptComInYourBrowser;

  /// No description provided for @newMeeting.
  ///
  /// In en, this message translates to:
  /// **'New meeting'**
  String get newMeeting;

  /// No description provided for @newMeetingExplanation.
  ///
  /// In en, this message translates to:
  /// **'A meeting without a recording. Write notes or add files to it: summaries and chat use them.'**
  String get newMeetingExplanation;

  /// No description provided for @titleOptional.
  ///
  /// In en, this message translates to:
  /// **'Title (optional)'**
  String get titleOptional;

  /// No description provided for @handoffStepPressEnter.
  ///
  /// In en, this message translates to:
  /// **'{site} opens with the meeting already in the message box. Press Enter to send it.'**
  String handoffStepPressEnter(String site);

  /// No description provided for @handoffStepSentByItself.
  ///
  /// In en, this message translates to:
  /// **'{site} opens and sends the meeting by itself. Sign in first if it asks.'**
  String handoffStepSentByItself(String site);

  /// No description provided for @handoffStepPaste.
  ///
  /// In en, this message translates to:
  /// **'{site} opens and Koalino copies the meeting. Click the message box, paste it (Ctrl+V) and press Enter.'**
  String handoffStepPaste(String site);

  /// No description provided for @handoffStepDragFiles.
  ///
  /// In en, this message translates to:
  /// **'A folder with the files opens too: drag them all into the message box, so they go as files.'**
  String get handoffStepDragFiles;

  /// No description provided for @handoffStepCopyAnswer.
  ///
  /// In en, this message translates to:
  /// **'When the answer is complete, press the copy button under it.'**
  String get handoffStepCopyAnswer;

  /// No description provided for @handoffStepComeBack.
  ///
  /// In en, this message translates to:
  /// **'Come back to Koalino: the answer becomes the meeting\'s summary, with Undo.'**
  String get handoffStepComeBack;

  /// No description provided for @handoffStepAskThere.
  ///
  /// In en, this message translates to:
  /// **'Then ask your questions about the meeting there.'**
  String get handoffStepAskThere;

  /// No description provided for @openSite.
  ///
  /// In en, this message translates to:
  /// **'Open {site}'**
  String openSite(String site);

  /// No description provided for @writtenBySourceOn.
  ///
  /// In en, this message translates to:
  /// **'Written by {source} on {createdAt}'**
  String writtenBySourceOn(String source, String createdAt);

  /// No description provided for @noRecordingWriteNotesOrAttach.
  ///
  /// In en, this message translates to:
  /// **'No recording. Write notes or attach files: summaries and chat use them.'**
  String get noRecordingWriteNotesOrAttach;

  /// No description provided for @handoffStepOpensFilled.
  ///
  /// In en, this message translates to:
  /// **'{site} opens with the meeting already in the message box.'**
  String handoffStepOpensFilled(String site);

  /// No description provided for @handoffStepOpensCopied.
  ///
  /// In en, this message translates to:
  /// **'{site} opens, and Koalino copies the meeting.'**
  String handoffStepOpensCopied(String site);

  /// No description provided for @handoffStepSend.
  ///
  /// In en, this message translates to:
  /// **'Press Enter to send it.'**
  String get handoffStepSend;

  /// No description provided for @handoffStepPasteSend.
  ///
  /// In en, this message translates to:
  /// **'Click the message box, paste the meeting (Ctrl+V) and press Enter.'**
  String get handoffStepPasteSend;

  /// Intro of the sheet that continues a meeting's or a project's chat in the owner's own AI app (#126).
  ///
  /// In en, this message translates to:
  /// **'Ask your questions in your own AI app, with your own account. Koalino gives it the notes and transcripts, and the photos and files go with them as files.'**
  String get chatHandoffIntro;
}

class _AppLocalizationsDelegate
    extends LocalizationsDelegate<AppLocalizations> {
  const _AppLocalizationsDelegate();

  @override
  Future<AppLocalizations> load(Locale locale) {
    return SynchronousFuture<AppLocalizations>(lookupAppLocalizations(locale));
  }

  @override
  bool isSupported(Locale locale) => <String>[
        'ar',
        'de',
        'en',
        'es',
        'fr',
        'it',
        'ja',
        'pt',
        'ru',
        'zh'
      ].contains(locale.languageCode);

  @override
  bool shouldReload(_AppLocalizationsDelegate old) => false;
}

AppLocalizations lookupAppLocalizations(Locale locale) {
  // Lookup logic when only language code is specified.
  switch (locale.languageCode) {
    case 'ar':
      return AppLocalizationsAr();
    case 'de':
      return AppLocalizationsDe();
    case 'en':
      return AppLocalizationsEn();
    case 'es':
      return AppLocalizationsEs();
    case 'fr':
      return AppLocalizationsFr();
    case 'it':
      return AppLocalizationsIt();
    case 'ja':
      return AppLocalizationsJa();
    case 'pt':
      return AppLocalizationsPt();
    case 'ru':
      return AppLocalizationsRu();
    case 'zh':
      return AppLocalizationsZh();
  }

  throw FlutterError(
      'AppLocalizations.delegate failed to load unsupported locale "$locale". This is likely '
      'an issue with the localizations generation tool. Please file an issue '
      'on GitHub with a reproducible sample app and the gen-l10n configuration '
      'that was used.');
}
