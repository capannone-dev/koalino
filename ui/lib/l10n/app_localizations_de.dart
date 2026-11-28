// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for German (`de`).
class AppLocalizationsDe extends AppLocalizations {
  AppLocalizationsDe([String locale = 'de']) : super(locale);

  @override
  String get unableToCheckTheAppLock =>
      'Die App-Sperre konnte nicht geprüft werden. Versuche es erneut.';

  @override
  String get unableToShowTheUnlockPrompt =>
      'Die Entsperrabfrage konnte nicht angezeigt werden. Versuche es erneut.';

  @override
  String get setAScreenLockOnThis =>
      'Richte zuerst eine Displaysperre auf diesem Gerät ein.';

  @override
  String get unableToChangeTheAppLock =>
      'Die App-Sperre konnte nicht geändert werden. Versuche es erneut.';

  @override
  String get aCallApp => 'Eine Anruf-App';

  @override
  String get call => 'Anruf';

  @override
  String call2(Object app) {
    return '$app-Anruf';
  }

  @override
  String theCallEndedSoTheRecording(Object tiedApp) {
    return 'Der $tiedApp-Anruf ist beendet, daher wurde die Aufnahme gestoppt.';
  }

  @override
  String get restartYourBrowserAndCallApps =>
      'Starte deinen Browser und deine Anruf-Apps einmal neu, damit Koalino lesen kann, wer in Anrufen spricht.';

  @override
  String get unableToLoadDevices => 'Geräte konnten nicht geladen werden';

  @override
  String get thisDeviceIsNotOnA =>
      'Dieses Gerät ist noch in keinem Netzwerk. Verbinde es und versuche es erneut.';

  @override
  String get thatDidNotWorkTryAgain =>
      'Das hat nicht funktioniert. Versuche es erneut.';

  @override
  String get use1To64CharactersWithout =>
      'Verwende 1 bis 64 Zeichen, ohne Zeilenumbrüche.';

  @override
  String get theNameCouldNotBeSaved =>
      'Der Name konnte nicht gespeichert werden. Versuche es erneut.';

  @override
  String get thisInviteHasExpiredMakeA =>
      'Diese Einladung ist abgelaufen. Erstelle auf dem anderen Gerät eine neue.';

  @override
  String get thisInviteWasMadeOnThis =>
      'Diese Einladung wurde auf diesem Gerät erstellt. Verwende sie auf dem anderen.';

  @override
  String get thatDeviceIsAlreadyPaired => 'Dieses Gerät ist bereits gekoppelt.';

  @override
  String get thatIsNotAValidInvite =>
      'Das ist keine gültige Einladung. Kopiere sie erneut.';

  @override
  String get deviceSyncIsNotRunning =>
      'Die Gerätesynchronisierung läuft nicht.';

  @override
  String get theOtherDeviceIsNoLonger =>
      'Das andere Gerät koppelt nicht mehr. Versuche es erneut.';

  @override
  String get theOtherAppCouldNotBe =>
      'Die andere App konnte nicht geöffnet werden.';

  @override
  String get theOtherApp => 'die andere App';

  @override
  String app(Object displayName) {
    return '$displayName-App';
  }

  @override
  String get unableToChangeTheLogLevel =>
      'Die Protokollstufe konnte nicht geändert werden';

  @override
  String logsExportedTo(Object filePath) {
    return 'Protokolle exportiert nach $filePath';
  }

  @override
  String get unableToShareTheLogs =>
      'Die Protokolle konnten nicht geteilt werden';

  @override
  String get noLogsToExportYet => 'Noch keine Protokolle zum Exportieren';

  @override
  String get unableToExportTheLogs =>
      'Die Protokolle konnten nicht exportiert werden';

  @override
  String couldNotBeAttached(Object attached, Object failedPaths) {
    return '$attached; $failedPaths konnten nicht angehängt werden';
  }

  @override
  String get theFilesCouldNotBeAttached =>
      'Die Dateien konnten nicht angehängt werden.';

  @override
  String get unableToShareTheExport => 'Der Export konnte nicht geteilt werden';

  @override
  String exportedTo(Object filePath) {
    return 'Exportiert nach $filePath';
  }

  @override
  String get exportSaved => 'Export gespeichert';

  @override
  String get unableToSaveTheExport =>
      'Der Export konnte nicht gespeichert werden';

  @override
  String get thisMeetingNoLongerExists => 'Dieses Meeting existiert nicht mehr';

  @override
  String get exportIsUnavailableUntilTheApp =>
      'Der Export ist erst verfügbar, wenn die App gestartet ist';

  @override
  String get unableToExportTheMeeting =>
      'Das Meeting konnte nicht exportiert werden';

  @override
  String get unableToLoadMeetings => 'Meetings konnten nicht geladen werden';

  @override
  String get unableToLoadThisMeeting =>
      'Dieses Meeting konnte nicht geladen werden';

  @override
  String get enterATitle => 'Gib einen Titel ein.';

  @override
  String titlesCanBeAtMostCharacters(
      Object maxMeetingTitleCharacters, Object runes) {
    return 'Titel dürfen höchstens $maxMeetingTitleCharacters Zeichen haben (du hast $runes).';
  }

  @override
  String get titlesCanTContainLineBreaks =>
      'Titel dürfen keine Zeilenumbrüche oder Steuerzeichen enthalten.';

  @override
  String get notPairedYetMakeTheRelay =>
      'Noch nicht gekoppelt. Mach das Relay sichtbar und kopple es dann unter Geräte in der Nähe in Geräte.';

  @override
  String get connectingToTheRelay => 'Verbindung zum Relay…';

  @override
  String get connectedToTheRelayAskingIt =>
      'Mit dem Relay verbunden. Es wird gebeten, Verbindungen anzunehmen…';

  @override
  String get connectedYourOtherDevicesCanReach =>
      'Verbunden. Deine anderen Geräte erreichen dieses über das Relay.';

  @override
  String get savedThisAppliesNowWithNo =>
      'Gespeichert. Gilt sofort, ohne Neustart.';

  @override
  String get thatRelayAddressIsNotValid =>
      'Diese Relay-Adresse ist ungültig. Sie sieht so aus: /ip4/203.0.113.5/tcp/4001/p2p/12D3Koo…';

  @override
  String get aDeviceAddressIsNotValid =>
      'Eine Geräteadresse ist ungültig. Jede endet mit /p2p/ und der Geräte-ID.';

  @override
  String get unableToSaveTheAddresses =>
      'Die Adressen konnten nicht gespeichert werden.';

  @override
  String get thePhotoCouldNotBeSaved =>
      'Das Foto konnte nicht gespeichert werden.';

  @override
  String get enterAName => 'Gib einen Namen ein.';

  @override
  String namesCanBeAtMostCharacters(Object maxMeetingTitleCharacters) {
    return 'Namen dürfen höchstens $maxMeetingTitleCharacters Zeichen haben.';
  }

  @override
  String get namesCanTContainLineBreaks =>
      'Namen dürfen keine Zeilenumbrüche oder Steuerzeichen enthalten.';

  @override
  String get unableToLoadProjects => 'Projekte konnten nicht geladen werden';

  @override
  String get unableToLoadThisProject =>
      'Dieses Projekt konnte nicht geladen werden';

  @override
  String get theFilesCouldNotBeAdded =>
      'Die Dateien konnten nicht hinzugefügt werden.';

  @override
  String couldNotBeAdded(Object failedPaths) {
    return '$failedPaths konnten nicht hinzugefügt werden';
  }

  @override
  String get unableToStartRecording => 'Aufnahme konnte nicht gestartet werden';

  @override
  String get unableToPauseRecording => 'Aufnahme konnte nicht pausiert werden';

  @override
  String get unableToResumeRecording =>
      'Aufnahme konnte nicht fortgesetzt werden';

  @override
  String get unableToStopRecording => 'Aufnahme konnte nicht gestoppt werden';

  @override
  String get recordingSaved => 'Aufnahme gespeichert';

  @override
  String get microphoneAccessIsNeededToRecord =>
      'Für die Aufnahme wird Mikrofonzugriff benötigt';

  @override
  String get keepTheAppOpenAndroidDid =>
      'Lass die App geöffnet: Android hat die Aufnahme im Hintergrund nicht erlaubt';

  @override
  String get unableToOpenTheMicrophone =>
      'Mikrofon konnte nicht geöffnet werden';

  @override
  String saved(Object label) {
    return '$label gespeichert.';
  }

  @override
  String removed(Object label) {
    return '$label entfernt.';
  }

  @override
  String isNowTheDefault(Object id) {
    return '$id ist jetzt der Standard.';
  }

  @override
  String answered(Object id, Object reply) {
    return '$id hat geantwortet: $reply';
  }

  @override
  String get promptSaved => 'Prompt gespeichert.';

  @override
  String get theBuiltInPromptIsBack =>
      'Der integrierte Prompt ist wiederhergestellt.';

  @override
  String get finishedMeetingsWillBeSummarizedAutomatically =>
      'Beendete Meetings werden automatisch zusammengefasst.';

  @override
  String get automaticSummariesAreOff =>
      'Automatische Zusammenfassungen sind aus.';

  @override
  String get thisComputerNoLongerWritesSummaries =>
      'Dieser Computer schreibt keine Zusammenfassungen mehr und beantwortet keine Chats mehr für deine anderen Geräte.';

  @override
  String nowWritesTheSummariesAndAnswers(Object id) {
    return '$id schreibt jetzt die Zusammenfassungen und beantwortet die Chats, die deine anderen Geräte anfragen.';
  }

  @override
  String get requestCancelled => 'Anfrage abgebrochen.';

  @override
  String get thisMeetingUsesTheSettingsPrompt =>
      'Dieses Meeting verwendet wieder den Prompt aus den Einstellungen.';

  @override
  String get promptSavedForThisMeeting =>
      'Prompt für dieses Meeting gespeichert.';

  @override
  String get summarySaved => 'Zusammenfassung gespeichert.';

  @override
  String get theConnection => 'Die Verbindung';

  @override
  String get summaryWritten => 'Zusammenfassung geschrieben.';

  @override
  String theTranscriptWasReadInParts(Object parts) {
    return ' Das Transkript wurde in $parts Teilen gelesen.';
  }

  @override
  String leftOut(Object sent, Object imagesSkipped) {
    return ' $sent, $imagesSkipped ausgelassen.';
  }

  @override
  String get theModelStoppedBeforeTheEnd =>
      ' Das Modell hat vor dem Ende aufgehört.';

  @override
  String get waitingForYourComputerItWrites =>
      'Warte auf deinen Computer. Er schreibt die Zusammenfassung, sobald er an, mit diesem Gerät verbunden und unter Einstellungen → KI-Verbindungen zum Antworten eingerichtet ist.';

  @override
  String get yourComputerIsWritingTheSummary =>
      'Dein Computer schreibt die Zusammenfassung…';

  @override
  String yourComputerCouldNotWriteThe(Object message) {
    return 'Dein Computer konnte die Zusammenfassung nicht schreiben: $message';
  }

  @override
  String get unableToRefreshTelemetry =>
      'Telemetrie konnte nicht aktualisiert werden';

  @override
  String get automatic => 'Automatisch';

  @override
  String get transcriptionLanguage => 'Transkriptionssprache';

  @override
  String get transcriptionLanguages => 'Transkriptionssprachen';

  @override
  String get theLanguageCouldNotBeChanged =>
      'Die Sprache konnte nicht geändert werden.';

  @override
  String get thisDevice => 'Dieses Gerät';

  @override
  String get bestAvailable => 'Bestes verfügbares';

  @override
  String get aRemovedDevice => 'Ein entferntes Gerät';

  @override
  String get theTranscriptionDeviceCouldNotBe =>
      'Das Transkriptionsgerät konnte nicht geändert werden. Prüfe, ob es noch gekoppelt ist.';

  @override
  String get downloadFinishedYouCanNowChoose =>
      'Download abgeschlossen. Du kannst dieses Modell jetzt wählen.';

  @override
  String get downloadCancelled => 'Download abgebrochen.';

  @override
  String get theDownloadStoppedNoConnectionTo =>
      'Der Download wurde gestoppt: keine Verbindung zum Server. Dein aktuelles Modell funktioniert weiterhin.';

  @override
  String get theDownloadedFileFailedItsCheck =>
      'Die heruntergeladene Datei hat die Prüfung nicht bestanden und wurde gelöscht. Dein aktuelles Modell funktioniert weiterhin.';

  @override
  String get theModelCouldNotBeSaved =>
      'Das Modell konnte nicht gespeichert werden. Prüfe den freien Speicher. Dein aktuelles Modell funktioniert weiterhin.';

  @override
  String get theDownloadServerRefusedTheFile =>
      'Der Download-Server hat die Datei abgelehnt. Dein aktuelles Modell funktioniert weiterhin.';

  @override
  String get savedTheNextRecordingUsesThis =>
      'Gespeichert. Die nächste Aufnahme verwendet dieses Modell.';

  @override
  String get savedTheNextRecordingUsesThis2 =>
      'Gespeichert. Die nächste Aufnahme verwendet dies.';

  @override
  String get savedTheNextRecordingUsesThe =>
      'Gespeichert. Die nächste Aufnahme verwendet die automatische Auswahl.';

  @override
  String get savedMeetingsRecordedFromNowOn =>
      'Gespeichert. Ab jetzt aufgenommene Meetings verwenden dies.';

  @override
  String get savedTheNextRecordingUsesThese =>
      'Gespeichert. Die nächste Aufnahme verwendet diese Namen und Begriffe.';

  @override
  String get eachTermCanHaveAtMost =>
      'Jeder Begriff darf höchstens 60 Zeichen haben, alle zusammen höchstens 500.';

  @override
  String get chooseAWindowBetween1And =>
      'Wähle einen Zeitraum zwischen 1 und 60 Tagen.';

  @override
  String get theDownloadCouldNotStartPlease =>
      'Der Download konnte nicht starten. Bitte versuche es erneut.';

  @override
  String get savedTheNextRecordingIsTranscribed =>
      'Gespeichert. Die nächste Aufnahme wird auf der NPU transkribiert.';

  @override
  String get savedTheNextRecordingIsTranscribed2 =>
      'Gespeichert. Die nächste Aufnahme wird auf dem Prozessor transkribiert.';

  @override
  String get theNpuFilesAreNotAll =>
      'Die NPU-Dateien sind noch nicht alle heruntergeladen.';

  @override
  String get theNpuFilesWereRemoved => 'Die NPU-Dateien wurden entfernt.';

  @override
  String get theNpuFilesCouldNotBe =>
      'Die NPU-Dateien konnten nicht entfernt werden. Bitte versuche es erneut.';

  @override
  String get downloadedRecordingsNowTranscribeOnThe =>
      'Heruntergeladen. Aufnahmen werden jetzt auf der NPU transkribiert.';

  @override
  String get theDownloadStoppedNoConnectionTo2 =>
      'Der Download wurde gestoppt: keine Verbindung zum Server.';

  @override
  String get aDownloadedFileFailedItsCheck =>
      'Eine heruntergeladene Datei hat die Prüfung nicht bestanden und wurde gelöscht.';

  @override
  String get theFilesCouldNotBeSaved =>
      'Die Dateien konnten nicht gespeichert werden. Prüfe den freien Speicher (etwa 800 MB werden benötigt).';

  @override
  String get theDownloadServerRefusedTheFiles =>
      'Der Download-Server hat die Dateien abgelehnt.';

  @override
  String get theNextRecordingTriesTheGraphics =>
      'Die nächste Aufnahme versucht es erneut mit dem Grafikchip.';

  @override
  String get thatModelWonTBeProposed =>
      'Dieses Modell wird nicht mehr vorgeschlagen.';

  @override
  String get modelRemoved => 'Modell entfernt.';

  @override
  String get anotherModelIsDownloadingWaitFor =>
      'Ein anderes Modell wird heruntergeladen. Warte, bis es fertig ist.';

  @override
  String get thereIsnTEnoughFreeSpace =>
      'Für dieses Modell ist nicht genug freier Speicher vorhanden.';

  @override
  String get thatDidnTWorkPleaseTry =>
      'Das hat nicht funktioniert. Bitte versuche es erneut.';

  @override
  String get unableToLoadVoiceGallery =>
      'Stimmengalerie konnte nicht geladen werden';

  @override
  String get unableToPlayThisVoice =>
      'Diese Stimme konnte nicht abgespielt werden';

  @override
  String get unableToDeleteThisVoice =>
      'Diese Stimme konnte nicht gelöscht werden';

  @override
  String get voiceDeleted => 'Stimme gelöscht';

  @override
  String get unableToDeleteThisPerson =>
      'Diese Person konnte nicht gelöscht werden';

  @override
  String deleted(Object displayName) {
    return '$displayName gelöscht';
  }

  @override
  String get enterANameBeforeAssigning =>
      'Gib vor dem Zuordnen einen Namen ein';

  @override
  String assignedTo(Object identityName) {
    return '$identityName zugeordnet';
  }

  @override
  String assignedToThereWasNotEnough(Object identityName) {
    return '$identityName zugeordnet. Es gab nicht genug deutliche Sprache, um diese Stimme in künftigen Aufnahmen zu erkennen.';
  }

  @override
  String get unableToAssignTheSpeakerCluster =>
      'Die Sprechergruppe konnte nicht zugeordnet werden';

  @override
  String get chooseAtLeastOneVoiceTo =>
      'Wähle mindestens eine Stimme zum Benennen';

  @override
  String thereWasNotEnoughClearSpeech(Object assigned) {
    return '$assigned. Es gab nicht genug deutliche Sprache, um diese Stimme in künftigen Aufnahmen zu erkennen.';
  }

  @override
  String get theSettingCouldNotBeSaved =>
      'Die Einstellung konnte nicht gespeichert werden.';

  @override
  String get theAppIsStillStartingTry =>
      'Die App startet noch. Versuche es erneut.';

  @override
  String get somethingWentWrongTryAgain =>
      'Etwas ist schiefgelaufen. Versuche es erneut.';

  @override
  String isUsingTheMicrophone(Object app) {
    return '$app verwendet das Mikrofon';
  }

  @override
  String get recordThisCall => 'Diesen Anruf aufnehmen?';

  @override
  String theAiServerRejectedTheRequest(Object detail) {
    return 'Der KI-Server hat die Anfrage abgelehnt.\nDer Server meldete: $detail';
  }

  @override
  String claudeCodeOrCodexCouldNot(Object detail) {
    return 'Claude Code oder Codex konnte nicht antworten.\nMeldung: $detail';
  }

  @override
  String get typeAQuestion => 'Gib eine Frage ein.';

  @override
  String get theQuestionIsTooLong => 'Die Frage ist zu lang.';

  @override
  String get addAnAiConnectionInSettings =>
      'Füge in den Einstellungen eine KI-Verbindung hinzu oder wähle eine Standardverbindung.';

  @override
  String get thatAiConnectionNoLongerExists =>
      'Diese KI-Verbindung existiert nicht mehr.';

  @override
  String get thereIsNothingToTalkAbout =>
      'Es gibt noch nichts zu besprechen: kein Transkript, keine Zusammenfassung, keine Notizen oder Dateien.';

  @override
  String get theAiConnectionSContextIs =>
      'Der Kontext der KI-Verbindung ist zu klein. Wähle eine mit größerem Kontext.';

  @override
  String get thatMeetingNoLongerExists =>
      'Dieses Meeting existiert nicht mehr.';

  @override
  String get thatProjectNoLongerExists =>
      'Dieses Projekt existiert nicht mehr.';

  @override
  String get theAiServerCouldNotBe =>
      'Der KI-Server war nicht erreichbar. Prüfe die Verbindung.';

  @override
  String get theAiServerRefusedTheApi =>
      'Der KI-Server hat den API-Schlüssel abgelehnt.';

  @override
  String get theAiServerIsBusyTry =>
      'Der KI-Server ist ausgelastet. Versuche es später erneut.';

  @override
  String get theModelDeclinedToAnswer =>
      'Das Modell hat die Antwort verweigert.';

  @override
  String get claudeCodeOrCodexIsNot =>
      'Claude Code oder Codex ist auf diesem Computer nicht installiert, oder die App findet es nicht.';

  @override
  String get theNotesCouldNotBeRead =>
      'Die Notizen konnten nicht gelesen werden.';

  @override
  String get noPairedComputerAnsweredOnA =>
      'Kein gekoppelter Computer hat geantwortet. Öffne auf einem Computer Einstellungen → KI, wähle unter „Zusammenfassungen für meine anderen Geräte“ eine Verbindung und lass ihn eingeschaltet und verbunden.';

  @override
  String get theComputerTookTooLongTo =>
      'Der Computer hat zu lange für die Antwort gebraucht.';

  @override
  String get theComputerDisconnectedBeforeAnsweringTry =>
      'Der Computer hat die Verbindung vor der Antwort getrennt. Versuche es erneut.';

  @override
  String get theComputerAnswersAtMost30 =>
      'Der Computer beantwortet höchstens 30 Fragen pro Stunde. Versuche es später erneut.';

  @override
  String get theComputerIsAnsweringOtherQuestions =>
      'Der Computer beantwortet gerade andere Fragen. Versuche es gleich noch einmal.';

  @override
  String get pairedDevicesAreOffOnThis =>
      'Gekoppelte Geräte sind auf diesem Telefon ausgeschaltet.';

  @override
  String get theAiServerCouldNotAnswer =>
      'Der KI-Server konnte nicht antworten. Versuche es erneut.';

  @override
  String get exportMeeting => 'Meeting exportieren';

  @override
  String get openaiCompatibleServer => 'OpenAI-kompatibler Server';

  @override
  String get localServerOllama => 'Lokaler Server (Ollama)';

  @override
  String get claudeCodeClaudePlan => 'Claude Code (Claude-Abo)';

  @override
  String get codexChatgptPlan => 'Codex (ChatGPT-Abo)';

  @override
  String get checkTheAddressAndModelPlain =>
      'Prüfe Adresse und Modell. Einfaches http:// funktioniert nur auf diesem Gerät oder im lokalen Netzwerk.';

  @override
  String get thatConnectionNoLongerExists =>
      'Diese Verbindung existiert nicht mehr.';

  @override
  String get addAnAiConnectionInSettings2 =>
      'Füge zuerst in den Einstellungen eine KI-Verbindung hinzu.';

  @override
  String get thisMeetingHasNoTranscriptOr =>
      'Dieses Meeting hat noch kein Transkript und keine Notizen.';

  @override
  String get thisMeetingIsTooLongFor =>
      'Dieses Meeting ist zu lang für diese Verbindung. Wähle eine mit größerem Kontext.';

  @override
  String get theAiServerCouldNotBe2 => 'Der KI-Server war nicht erreichbar.';

  @override
  String get theAiServerRefusedAccessUse =>
      'Der KI-Server hat den Zugriff verweigert. Prüfe den API-Schlüssel der Verbindung.';

  @override
  String get theAiServerIsBusyOr =>
      'Der KI-Server ist ausgelastet oder über seinem Limit. Versuche es später erneut.';

  @override
  String get theAiServerRejectedTheRequest2 =>
      'Der KI-Server hat die Anfrage abgelehnt. Prüfe den Modellnamen.';

  @override
  String get theAiServerFailedTryAgain =>
      'Beim KI-Server ist ein Fehler aufgetreten. Versuche es erneut.';

  @override
  String get theAiServerSAnswerCould =>
      'Die Antwort des KI-Servers konnte nicht gelesen werden.';

  @override
  String get claudeCodeOrCodexCouldNot2 =>
      'Claude Code oder Codex konnte nicht antworten. Prüfe, ob es angemeldet ist.';

  @override
  String get theSettingsCouldNotBeSaved =>
      'Die Einstellungen konnten nicht gespeichert werden.';

  @override
  String get theMeetingCouldNotBeRead =>
      'Das Meeting konnte nicht gelesen oder gespeichert werden.';

  @override
  String get error => 'Fehler';

  @override
  String get warning => 'Warnung';

  @override
  String get infoDefault => 'Info (Standard)';

  @override
  String get debug => 'Debug';

  @override
  String get trace => 'Trace';

  @override
  String get thisDevice2 => 'dieses Gerät';

  @override
  String get anotherDevice => 'ein anderes Gerät';

  @override
  String notConnected(Object label) {
    return '$label (nicht verbunden)';
  }

  @override
  String itIsKeptEncryptedUntil(Object until) {
    return ' Sie wird bis $until verschlüsselt aufbewahrt.';
  }

  @override
  String get recordedOnThisDeviceOnly => 'Nur auf diesem Gerät aufgenommen.';

  @override
  String recordedOnThisDeviceOnlyA(Object until) {
    return 'Nur auf diesem Gerät aufgenommen. Eine Aufnahme davon auf einem anderen Gerät kann noch bis $until kombiniert werden.';
  }

  @override
  String combinesTheRecordingsNothingIsLeft(Object combiner) {
    return '$combiner kombiniert die Aufnahmen; von hier ist nichts mehr zu senden.';
  }

  @override
  String waitingForAudioFrom(Object names, Object kept) {
    return 'Warte auf Audio von $names.$kept';
  }

  @override
  String combinesTheRecordingsWaitingForAudio(Object combiner, Object names) {
    return '$combiner kombiniert die Aufnahmen; warte auf Audio von $names.';
  }

  @override
  String get audioFromEveryDeviceIsHere =>
      'Audio von allen Geräten ist da; das Transkript wird kombiniert, wenn dieses Gerät frei ist.';

  @override
  String thisDeviceSAudioStillHas(Object combiner, Object kept) {
    return 'Das Audio dieses Geräts muss noch $combiner erreichen.$kept';
  }

  @override
  String get transcriptCombinedFromEveryDevice =>
      'Transkript aus allen Geräten kombiniert.';

  @override
  String notCombinedAudioFromDidNot(Object plainNames) {
    return 'Nicht kombiniert: Audio von $plainNames kam nicht innerhalb von drei Wochen an.';
  }

  @override
  String meeting(Object createdAt) {
    return 'Meeting $createdAt';
  }

  @override
  String get writeSomethingFirst => 'Schreib zuerst etwas.';

  @override
  String get theTextIsTooLong => 'Der Text ist zu lang.';

  @override
  String get thatPartOfTheTranscriptChanged =>
      'Dieser Teil des Transkripts hat sich inzwischen geändert. Öffne das Meeting erneut.';

  @override
  String get theMeetingCouldNotBeSaved =>
      'Das Meeting konnte nicht gespeichert werden.';

  @override
  String get chooseAFullFolderPathInside =>
      'Wähle einen vollständigen Ordnerpfad innerhalb deines Home-Ordners, aber nicht den Home-Ordner selbst.';

  @override
  String get theFolderCouldNotBeWritten =>
      'In den Ordner konnte nicht geschrieben werden. Prüfe, ob er existiert und du dort schreiben darfst.';

  @override
  String get theNotesCouldNotBeRead2 =>
      'Die Notizen konnten nicht gelesen oder gespeichert werden.';

  @override
  String get theInviteAnswerIsIncomplete =>
      'Die Antwort auf die Einladung ist unvollständig.';

  @override
  String get theRenameAnswerIsIncomplete =>
      'Die Antwort auf die Umbenennung ist unvollständig.';

  @override
  String get thisDeviceHasNoCamera => 'Dieses Gerät hat keine Kamera.';

  @override
  String get theCameraIsNotOpen => 'Die Kamera ist nicht geöffnet.';

  @override
  String get title => 'Titel';

  @override
  String get summary => 'Zusammenfassung';

  @override
  String get notes => 'Notizen';

  @override
  String get transcript => 'Transkript';

  @override
  String get projectName => 'Projektname';

  @override
  String file(Object fileName) {
    return 'Datei $fileName';
  }

  @override
  String get typeWhatToLookFor => 'Gib ein, wonach gesucht werden soll.';

  @override
  String get typeAtLeastTwoLetters => 'Gib mindestens zwei Buchstaben ein.';

  @override
  String get theSearchIsTooLong => 'Die Suche ist zu lang.';

  @override
  String get exportLogs => 'Protokolle exportieren';

  @override
  String about(Object appName) {
    return 'Über $appName';
  }

  @override
  String get versionLicenseAndTheSoftwareIt =>
      'Version, Lizenz und verwendete Software.';

  @override
  String remove(Object label) {
    return '$label entfernen?';
  }

  @override
  String get itsApiKeyIsDeletedFrom =>
      'Ihr API-Schlüssel wird von diesem Gerät gelöscht. Bereits geschriebene Zusammenfassungen bleiben.';

  @override
  String get cancel => 'Abbrechen';

  @override
  String get remove2 => 'Entfernen';

  @override
  String get aiConnections => 'KI-Verbindungen';

  @override
  String get addConnection => 'Verbindung hinzufügen';

  @override
  String get summariesSendTheMeetingSTranscript =>
      'Zusammenfassungen senden Transkript, Notizen und Fotos des Meetings an den Server der Verbindung. Ein Server in deinem lokalen Netzwerk behält sie dort.';

  @override
  String get noConnectionsYet => 'Noch keine Verbindungen';

  @override
  String get addAServerOnYourNetwork =>
      'Füge einen Server in deinem Netzwerk hinzu (Ollama, LM Studio, llama.cpp) oder Claude Code bzw. Codex auf diesem Computer.';

  @override
  String get summarizeMeetingsAutomatically =>
      'Meetings automatisch zusammenfassen';

  @override
  String get withTheDefaultConnectionWhenA =>
      'Mit der Standardverbindung, sobald das Transkript eines Meetings endgültig ist.';

  @override
  String get summariesForMyOtherDevices =>
      'Zusammenfassungen für meine anderen Geräte';

  @override
  String get aPairedPhoneCanAskThis =>
      'Ein gekoppeltes Telefon kann diesen Computer bitten, mit dieser Verbindung eine Zusammenfassung zu schreiben oder einen Chat zu beantworten, anhand der Kopie deiner Notizen auf diesem Computer. Das zählt für dieses Abo; höchstens 10 Zusammenfassungen und 30 Chat-Antworten pro Stunde.';

  @override
  String get off => 'Aus';

  @override
  String get summaryPrompt => 'Prompt für Zusammenfassungen';

  @override
  String placeholdersTheTranscriptIsAlwaysSent(Object placeholders) {
    return 'Platzhalter: $placeholders. Das Transkript wird immer nach dem Prompt gesendet.';
  }

  @override
  String get savePrompt => 'Prompt speichern';

  @override
  String get resetToBuiltIn => 'Auf integrierten zurücksetzen';

  @override
  String get defaultLabel => 'Standard';

  @override
  String get installed => 'Installiert';

  @override
  String get notFoundOnThisComputer => 'Auf diesem Computer nicht gefunden';

  @override
  String get unencryptedHttpLocalNetworkOnly =>
      'Unverschlüsseltes http, nur lokales Netzwerk';

  @override
  String get runsWithYourPlanSSign =>
      'Läuft mit der Anmeldung deines Abos auf diesem Computer';

  @override
  String actionsFor(Object label) {
    return 'Aktionen für $label';
  }

  @override
  String get makeDefault => 'Als Standard festlegen';

  @override
  String get test => 'Testen';

  @override
  String get edit => 'Bearbeiten';

  @override
  String get enterANameAnAddressAnd =>
      'Gib einen Namen, eine Adresse und ein Modell ein.';

  @override
  String get theContextIsANumberOf =>
      'Der Kontext ist eine Anzahl von Tokens, etwa 8192.';

  @override
  String edit2(Object label) {
    return '$label bearbeiten';
  }

  @override
  String get name => 'Name';

  @override
  String get protocol => 'Protokoll';

  @override
  String get runsTheToolInstalledAndSigned =>
      'Nutzt das auf diesem Computer installierte und angemeldete Tool, sodass dein Claude- oder ChatGPT-Abo die Zusammenfassungen schreibt statt API-Guthaben. Das Meeting geht unter diesem Konto an Anthropic oder OpenAI.';

  @override
  String get address => 'Adresse';

  @override
  String get httpsAlwaysHttpOnlyOnThis =>
      'https:// immer; http:// nur auf diesem Gerät oder im lokalen Netzwerk.';

  @override
  String get model => 'Modell';

  @override
  String get leaveEmptyForTheToolS => 'Leer lassen für den Standard des Tools.';

  @override
  String get contextTokens => 'Kontext (Tokens)';

  @override
  String get saveConnection => 'Verbindung speichern';

  @override
  String get koalinoIsLocked => 'Koalino ist gesperrt';

  @override
  String get unlockWithYourFingerprintFaceOr =>
      'Entsperre mit Fingerabdruck, Gesicht oder Displaysperre.';

  @override
  String get unlock => 'Entsperren';

  @override
  String copied(Object what) {
    return '$what kopiert';
  }

  @override
  String get aiAssistantAccess => 'Zugriff für KI-Assistenten';

  @override
  String get letYourOwnAiAssistantSuch =>
      'Lass deinen eigenen KI-Assistenten, etwa Claude Code, Codex oder Claude Desktop, deine Meetings und Projekte lesen: Titel, Zusammenfassungen, Notizen, Transkripte und Projektdateien. Er kann nur lesen; er kann nichts ändern oder löschen.';

  @override
  String get whatTheAssistantReadsMayBe =>
      'Was der Assistent liest, kann unter deinem Konto bei diesem Dienst an sein Online-Modell gesendet werden. Schalte dies nur für einen Assistenten ein, dem du deine Meetings anvertraust.';

  @override
  String get theAssistantServerIsNotInstalled =>
      'Der Assistenten-Server ist mit dieser Kopie der App nicht installiert.';

  @override
  String get letMyAiAssistantReadMy =>
      'Meinen KI-Assistenten meine Notizen lesen lassen';

  @override
  String get connectYourAssistant => 'Assistenten verbinden';

  @override
  String get yourNotesAreUnlockedWithA =>
      'Deine Notizen werden mit einer Passphrase entsperrt, die nie in die Einstellungen eines Assistenten geschrieben wird. Der Server liest sie nur, wenn NOTE_TAKER_ENCRYPTION_PASSPHRASE dort gesetzt ist, wo der Assistent ihn startet.';

  @override
  String get claudeCodeRunInATerminal =>
      'Claude Code: in einem Terminal ausführen';

  @override
  String get command => 'Befehl';

  @override
  String get claudeDesktopAndOtherMcpClients =>
      'Claude Desktop und andere MCP-Clients: zu ihren Einstellungen hinzufügen';

  @override
  String get settings => 'Einstellungen';

  @override
  String copy(Object title) {
    return 'Kopieren: $title';
  }

  @override
  String isUsingTheMicrophoneRecordThis(Object app) {
    return '$app verwendet das Mikrofon. Diesen Anruf aufnehmen?';
  }

  @override
  String get notNow => 'Nicht jetzt';

  @override
  String get record => 'Aufnehmen';

  @override
  String get offerToRecordCalls => 'Aufnahme von Anrufen anbieten';

  @override
  String get whenTeamsZoomMeetInA =>
      'Wenn Teams, Zoom, Meet im Browser oder eine andere Anruf-App das Mikrofon nutzt. Nichts wird aufgenommen, bis du auf Aufnehmen tippst.';

  @override
  String get stopWhenTheCallEnds => 'Beenden, wenn der Anruf endet';

  @override
  String get onlyForRecordingsStartedFromThe =>
      'Nur für Aufnahmen, die über das Angebot gestartet wurden, sobald die Anruf-App das Mikrofon 20 Sekunden lang freigegeben hat.';

  @override
  String get readSpeakerNamesFromTheCall =>
      'Namen der Sprechenden aus dem Anruffenster lesen';

  @override
  String get whileRecordingKoalinoReadsWhoTeams =>
      'Während der Aufnahme liest Koalino, wen Teams, Zoom oder Meet als sprechend anzeigt, um Namen für unbekannte Stimmen vorzuschlagen. Nichts verlässt diesen Computer, und niemand wird benannt, bis du auf Zuordnen tippst.';

  @override
  String get deleteThisChat => 'Diesen Chat löschen?';

  @override
  String get itsQuestionsAndAnswersAreDeleted =>
      'Seine Fragen und Antworten werden von diesem Gerät gelöscht.';

  @override
  String get delete => 'Löschen';

  @override
  String get thatFileIsNoLongerIn => 'Diese Datei ist nicht mehr im Projekt.';

  @override
  String get whoAnswers => 'Wer antwortet';

  @override
  String get myPairedComputer => 'Mein gekoppelter Computer';

  @override
  String get thisPhoneSAiConnection => 'Die KI-Verbindung dieses Telefons';

  @override
  String get deleteThisChat2 => 'Diesen Chat löschen';

  @override
  String get askAboutYourNotesAnswersCome =>
      'Frag zu deinen Notizen. Die Antworten kommen von deinem gekoppelten Computer, der seine eigene Kopie deiner Notizen mit der dort für deine anderen Geräte gewählten Verbindung liest.';

  @override
  String get askAboutYourNotesTheAnswer =>
      'Frag zu deinen Notizen. Die Antwort kommt von deiner Standard-KI-Verbindung, an die die passenden Notizen gesendet werden.';

  @override
  String get yourQuestion => 'Deine Frage';

  @override
  String get ask => 'Fragen';

  @override
  String get open => 'Öffnen';

  @override
  String given(Object included) {
    return 'Übergeben: $included';
  }

  @override
  String leftOutForLength(Object omitted) {
    return 'Aus Platzgründen weggelassen: $omitted';
  }

  @override
  String answeredBy(Object connection) {
    return 'Beantwortet von $connection';
  }

  @override
  String get unableToOpenTheKeychain =>
      'Der Schlüsselbund konnte nicht geöffnet werden';

  @override
  String get koalinoKeepsTheKeyThatEncrypts =>
      'Koalino bewahrt den Schlüssel, der deine Stimmprofile und die Geräteidentität verschlüsselt, im Schlüsselbund des Systems auf. Entsperre den Schlüsselbund oder erlaube Koalino, ihn zu nutzen, und versuche es erneut. Hat dieses Gerät keinen Schlüsselbunddienst, installiere oder starte einen und starte Koalino neu.';

  @override
  String get unableToOpenYourNotes =>
      'Deine Notizen konnten nicht geöffnet werden';

  @override
  String get koalinoCouldNotOpenItsDatabase =>
      'Koalino konnte seine Datenbank nicht öffnen. Prüfe, ob genug freier Speicher vorhanden ist, und versuche es erneut.';

  @override
  String get unlockingYourNotes => 'Deine Notizen werden entsperrt…';

  @override
  String get quit => 'Beenden';

  @override
  String get retry => 'Erneut versuchen';

  @override
  String delete2(Object title) {
    return '$title löschen?';
  }

  @override
  String get itsTranscriptAttachmentsAndUnknownVoices =>
      'Transkript, Anhänge und unbekannte Stimmen werden endgültig gelöscht, hier und auf deinen gekoppelten Geräten. Gespeicherte Personen bleiben.';

  @override
  String get unableToDeleteThisMeetingIf =>
      'Dieses Meeting konnte nicht gelöscht werden. Wenn es gerade aufgenommen wird, beende zuerst die Aufnahme.';

  @override
  String get devices => 'Geräte';

  @override
  String get deviceSyncIsOff => 'Die Gerätesynchronisierung ist aus';

  @override
  String get meetingsStayOnThisDeviceUntil =>
      'Meetings bleiben auf diesem Gerät, bis die Synchronisierung eingeschaltet wird.';

  @override
  String thisDevice3(Object capabilities) {
    return 'Dieses Gerät\n$capabilities';
  }

  @override
  String get renameThisDevice => 'Dieses Gerät umbenennen';

  @override
  String get discoverableYourOtherDevicesCanAsk =>
      'Sichtbar: Deine anderen Geräte können einige Minuten lang die Kopplung anfragen.';

  @override
  String get makeDiscoverable => 'Sichtbar machen';

  @override
  String get pairingRequests => 'Kopplungsanfragen';

  @override
  String get acceptOnlyIfTheOtherDevice =>
      'Nimm nur an, wenn das andere Gerät denselben Code zeigt.';

  @override
  String get reject => 'Ablehnen';

  @override
  String get accept => 'Annehmen';

  @override
  String get pairingInProgress => 'Kopplung läuft';

  @override
  String get waitingForTheOtherDevice => 'Warte auf das andere Gerät…';

  @override
  String get checkThatTheOtherDeviceShows =>
      'Prüfe, ob das andere Gerät denselben Code zeigt.';

  @override
  String get cancelPairing => 'Kopplung abbrechen';

  @override
  String get codesMatch => 'Codes stimmen überein';

  @override
  String get pairedDevices => 'Gekoppelte Geräte';

  @override
  String get noPairedDevicesYet => 'Noch keine gekoppelten Geräte.';

  @override
  String paired(Object pairedAt, Object hasRelay) {
    return 'Gekoppelt am $pairedAt\n$hasRelay';
  }

  @override
  String remove3(Object deviceName) {
    return '$deviceName entfernen';
  }

  @override
  String get nearbyDevices => 'Geräte in der Nähe';

  @override
  String get pair => 'Koppeln';

  @override
  String get anotherNetwork => 'Ein anderes Netzwerk';

  @override
  String get showAnInviteHereAndUse =>
      'Zeige hier eine Einladung und verwende sie auf dem anderen Gerät. Vor dem Koppeln vergleichst du trotzdem einen Code.';

  @override
  String get showInvite => 'Einladung zeigen';

  @override
  String get useInvite => 'Einladung verwenden';

  @override
  String stopsSyncingWithThisDevicePair(Object deviceName) {
    return '$deviceName synchronisiert nicht mehr mit diesem Gerät. Kopple es erneut, um fortzufahren.';
  }

  @override
  String pairingCode(Object split) {
    return 'Kopplungscode $split';
  }

  @override
  String get noOtherDevicesFoundOnThis =>
      'In diesem Netzwerk wurden keine anderen Geräte gefunden. In der Nähe findet nur Geräte im selben WLAN. Für ein Gerät in einem anderen Netzwerk verwende eine Einladung.';

  @override
  String get noOtherDevicesFoundOnThis2 =>
      'In diesem Netzwerk wurden keine anderen Geräte gefunden. In der Nähe findet nur Geräte im selben WLAN. Für ein Gerät in einem anderen Netzwerk verwende eine Einladung oder lege in den Einstellungen ein Relay fest.';

  @override
  String get notConnectedItReconnectsWhenIt =>
      'Nicht verbunden. Es verbindet sich wieder, sobald es online ist und dein Relay erreicht.';

  @override
  String get notConnectedPutBothDevicesOn =>
      'Nicht verbunden. Bring beide Geräte ins selbe WLAN oder lege in den Einstellungen ein Relay fest.';

  @override
  String get whisperOnDevice => 'Whisper (auf dem Gerät)';

  @override
  String get openaiCloud => 'OpenAI (Cloud)';

  @override
  String gbGpu(Object round) {
    return '$round GB GPU';
  }

  @override
  String mbGpu(Object gpuMegabytes) {
    return '$gpuMegabytes MB GPU';
  }

  @override
  String get noTranscriptionModels => 'Keine Transkriptionsmodelle';

  @override
  String get useYourOwnAiApp => 'Eigene KI-App verwenden';

  @override
  String get theMeetingItsNotesAndIts =>
      'Das Meeting, seine Notizen und Dateien gehen mit deinem eigenen Konto an die App, die du wählst. Für eine Zusammenfassung kopiere dort die Antwort und komm zurück, oder teile sie mit Koalino: Koalino bewahrt sie auf.';

  @override
  String summarizeIn(Object displayName) {
    return 'In $displayName zusammenfassen';
  }

  @override
  String chatIn(Object displayName) {
    return 'In $displayName chatten';
  }

  @override
  String get summarizeInAnotherApp => 'In einer anderen App zusammenfassen…';

  @override
  String get chatInAnotherApp => 'In einer anderen App chatten…';

  @override
  String summaryFromSavedTo(Object appName, Object title) {
    return 'Zusammenfassung aus $appName in „$title“ gespeichert.';
  }

  @override
  String get undo => 'Rückgängig';

  @override
  String get theEarlierSummaryIsBack =>
      'Die vorherige Zusammenfassung ist wiederhergestellt.';

  @override
  String toKeepTheSummaryCopyS(Object appName) {
    return 'Um die Zusammenfassung zu behalten, kopiere die Antwort von $appName und komm zu Koalino zurück, oder teile die Antwort mit Koalino.';
  }

  @override
  String get meetings => 'Meetings';

  @override
  String get projects => 'Projekte';

  @override
  String get voices => 'Stimmen';

  @override
  String get meeting2 => 'Meeting';

  @override
  String get chatAboutThisMeeting => 'Über dieses Meeting chatten';

  @override
  String chat(Object meeting) {
    return 'Chat: $meeting';
  }

  @override
  String get thisMeeting => 'dieses Meeting';

  @override
  String get takePhotos => 'Fotos aufnehmen';

  @override
  String get attachFiles => 'Dateien anhängen';

  @override
  String get export => 'Exportieren';

  @override
  String shareAs(Object label) {
    return 'Als $label teilen';
  }

  @override
  String get nameSpeakers => 'Sprechende benennen';

  @override
  String get selectAMeetingToSeeIts =>
      'Wähle ein Meeting, um sein Transkript zu sehen.';

  @override
  String get renameMeeting => 'Meeting umbenennen';

  @override
  String get deleteMeeting => 'Meeting löschen';

  @override
  String recordedOn(Object devices) {
    return 'Aufgenommen auf $devices';
  }

  @override
  String get split => 'Trennen';

  @override
  String get longPressOrRightClickA =>
      'Halte eine Passage gedrückt oder klicke sie mit rechts an, um sie zu korrigieren.';

  @override
  String get noTranscriptIsAvailable => 'Kein Transkript verfügbar.';

  @override
  String get theSpeakerIsAGuess => 'Die sprechende Person ist geschätzt';

  @override
  String get peopleTalkedAtOnceHereWords =>
      'Hier sprachen mehrere gleichzeitig; Wörter können fehlen';

  @override
  String get overlappingSpeech => 'Überlappende Sprache';

  @override
  String get someWordsWereUnclear => 'Einige Wörter waren undeutlich';

  @override
  String get addedAfterTheMeeting => 'Nach dem Meeting hinzugefügt';

  @override
  String photoOpenFullScreen(Object fileName) {
    return 'Foto $fileName, im Vollbild öffnen';
  }

  @override
  String get save => 'Speichern';

  @override
  String get preview => 'Vorschau';

  @override
  String get editNotes => 'Notizen bearbeiten';

  @override
  String get writeNotes => 'Notizen schreiben';

  @override
  String get loading => 'Wird geladen…';

  @override
  String get noNotesYet => 'Noch keine Notizen.';

  @override
  String textAt(Object startTimeMs) {
    return 'Text bei $startTimeMs';
  }

  @override
  String get removedThisTranscriptWonTBe =>
      'Entfernt. Dieses Transkript wird nach dem Meeting nicht neu transkribiert.';

  @override
  String get savedThisTranscriptWonTBe =>
      'Gespeichert. Dieses Transkript wird nach dem Meeting nicht neu transkribiert.';

  @override
  String get editText => 'Text bearbeiten';

  @override
  String get editTheTranscript => 'Transkript bearbeiten';

  @override
  String get leaveItEmptyToRemoveThis =>
      'Leer lassen, um diesen Teil zu entfernen.';

  @override
  String get searchMeetings => 'Meetings durchsuchen';

  @override
  String get chatWithAllYourNotes => 'Mit allen deinen Notizen chatten';

  @override
  String get chatWithAllNotes => 'Mit allen Notizen chatten';

  @override
  String get refresh => 'Aktualisieren';

  @override
  String get noMeetingsYet => 'Noch keine Meetings';

  @override
  String get recordingsYouStartAppearHere =>
      'Aufnahmen, die du startest, erscheinen hier.';

  @override
  String get rename => 'Umbenennen';

  @override
  String get addToProject => 'Zu Projekt hinzufügen…';

  @override
  String get summary2 => 'Zusammenfassung…';

  @override
  String get addToProject2 => 'Zu Projekt hinzufügen';

  @override
  String writingTheSummaryOf(Object title) {
    return 'Zusammenfassung von $title wird geschrieben';
  }

  @override
  String summaryOf(Object title) {
    return 'Zusammenfassung von $title';
  }

  @override
  String summarizeWith(Object label) {
    return 'Mit $label zusammenfassen';
  }

  @override
  String defaultLabel2(Object model) {
    return '$model · Standard';
  }

  @override
  String get summarizeOnMyComputer => 'Auf meinem Computer zusammenfassen';

  @override
  String get withClaudeCodeCodexOrThe =>
      'Mit Claude Code, Codex oder der dort eingerichteten Verbindung';

  @override
  String get useYourOwnAiApp2 => 'Eigene KI-App verwenden…';

  @override
  String get claudeChatgptOrAnotherAppWith =>
      'Claude, ChatGPT oder eine andere App, mit deinem Konto';

  @override
  String get openSummary => 'Zusammenfassung öffnen';

  @override
  String get editPromptForThisMeeting =>
      'Prompt für dieses Meeting bearbeiten…';

  @override
  String get useTheSettingsPrompt => 'Prompt aus den Einstellungen verwenden';

  @override
  String get promptForThisMeeting => 'Prompt für dieses Meeting';

  @override
  String placeholders(Object placeholders) {
    return 'Platzhalter: $placeholders';
  }

  @override
  String get addAnAiConnectionInSettings3 =>
      'Füge in den Einstellungen eine KI-Verbindung hinzu, um Zusammenfassungen zu schreiben.';

  @override
  String get summarizeWith2 => 'Zusammenfassen mit';

  @override
  String get summarize => 'Zusammenfassen';

  @override
  String get summarizeAgain => 'Erneut zusammenfassen';

  @override
  String get editPromptForThisMeeting2 =>
      'Prompt für dieses Meeting bearbeiten';

  @override
  String get writingTheSummaryALocalModel =>
      'Zusammenfassung wird geschrieben… Ein lokales Modell kann einige Minuten brauchen.';

  @override
  String get noSummaryYet => 'Noch keine Zusammenfassung.';

  @override
  String get writeASummary => 'Zusammenfassung schreiben';

  @override
  String writtenByYouOn(Object createdAt) {
    return 'Von dir geschrieben am $createdAt';
  }

  @override
  String writtenByOn(
      Object model, Object connectionLabel, Object createdAt, Object edited) {
    return 'Geschrieben von $model ($connectionLabel) am $createdAt$edited';
  }

  @override
  String get editSummary => 'Zusammenfassung bearbeiten';

  @override
  String get tryAgainOnMyComputer => 'Auf meinem Computer erneut versuchen';

  @override
  String get cancelRequest => 'Anfrage abbrechen';

  @override
  String get notesFolder => 'Notizordner';

  @override
  String get keepYourProjectsAndMeetingsAs =>
      'Halte deine Projekte und Meetings als einfache Dateien in einem Ordner, um sie mit jedem Editor zu lesen und zu bearbeiten. Änderungen an notes.md, summary.md und Projektdateien kommen innerhalb einer halben Minute in die App zurück, und Dateien, die du in einen Projektordner oder den Anhangordner eines Meetings legst, werden hinzugefügt. transcript.md schreibt die App neu: Korrigiere das Transkript in der App.';

  @override
  String get theFolderIsADecryptedCopy =>
      'Der Ordner ist eine entschlüsselte Kopie deiner Notizen. Wer ihn lesen kann, kann sie lesen, und Backup- oder Sync-Tools kopieren ihn. Bewahre ihn auf einem verschlüsselten Datenträger in deinem eigenen Benutzerordner auf.';

  @override
  String get folder => 'Ordner';

  @override
  String get chooseAFolder => 'Ordner wählen';

  @override
  String get keepTheNotesFolder => 'Notizordner führen';

  @override
  String get syncNow => 'Jetzt synchronisieren';

  @override
  String lastSyncAtFailed(Object at, Object code) {
    return 'Letzte Synchronisierung um $at fehlgeschlagen: $code';
  }

  @override
  String lastSyncAtFilesWrittenEdits(
      Object at, Object written, Object imported) {
    return 'Letzte Synchronisierung um $at: $written Dateien geschrieben, $imported Änderungen und Dateien übernommen.';
  }

  @override
  String get inviteADevice => 'Ein Gerät einladen';

  @override
  String get pairingInviteQrCode => 'QR-Code der Kopplungseinladung';

  @override
  String onTheOtherDeviceOpenDevices(Object hour, Object minute) {
    return 'Öffne auf dem anderen Gerät Geräte, wähle Einladung verwenden und füge diesen Link ein. Du kannst den Code mit einer Kamera-App scannen, um ihn zu kopieren. Er gilt bis $hour:$minute.';
  }

  @override
  String get close => 'Schließen';

  @override
  String get inviteLinkCopied => 'Einladungslink kopiert';

  @override
  String get copyLink => 'Link kopieren';

  @override
  String get deviceFoundChoosePairUnderNearby =>
      'Gerät gefunden. Wähle Koppeln unter Geräte in der Nähe.';

  @override
  String get useAnInvite => 'Eine Einladung verwenden';

  @override
  String get inviteLink => 'Einladungslink';

  @override
  String get findDevice => 'Gerät suchen';

  @override
  String get pairWithAnotherDevice => 'Mit einem anderen Gerät koppeln?';

  @override
  String get youOpenedAPairingInviteKoalino =>
      'Du hast eine Kopplungseinladung geöffnet. Koalino sucht das Gerät, das sie zeigt, und beide Geräte zeigen einen Code zum Vergleichen, bevor etwas geteilt wird.';

  @override
  String get pairingStartedCheckThatBothDevices =>
      'Kopplung gestartet. Prüfe, ob beide Geräte denselben Code zeigen.';

  @override
  String get done => 'Fertig';

  @override
  String get photosAreAddedToTheMeeting =>
      'Fotos werden dem Meeting hinzugefügt, während du sie aufnimmst.';

  @override
  String get takePhoto => 'Foto aufnehmen';

  @override
  String get theCameraIsUnavailable => 'Die Kamera ist nicht verfügbar.';

  @override
  String search(Object name) {
    return '$name durchsuchen';
  }

  @override
  String get noteSaved => 'Notiz gespeichert';

  @override
  String get theNoteCouldNotBeSaved =>
      'Die Notiz konnte nicht gespeichert werden.';

  @override
  String couldNotBeAdded2(Object failed) {
    return '$failed konnten nicht hinzugefügt werden.';
  }

  @override
  String get renameProject => 'Projekt umbenennen';

  @override
  String get theProjectCouldNotBeRenamed =>
      'Das Projekt konnte nicht umbenannt werden.';

  @override
  String delete3(Object project) {
    return '„$project“ löschen?';
  }

  @override
  String get itsFilesNotesAndEMails =>
      'Seine Dateien, Notizen und E-Mails werden auf allen gekoppelten Geräten gelöscht. Seine Meetings bleiben erhalten, außerhalb jedes Projekts.';

  @override
  String get theProjectCouldNotBeDeleted =>
      'Das Projekt konnte nicht gelöscht werden.';

  @override
  String get itIsDeletedOnEveryPaired =>
      'Es wird auf allen gekoppelten Geräten gelöscht.';

  @override
  String get theFileCouldNotBeDeleted =>
      'Die Datei konnte nicht gelöscht werden.';

  @override
  String get theMeetingCouldNotBeRemoved =>
      'Das Meeting konnte nicht aus dem Projekt entfernt werden.';

  @override
  String get project => 'Projekt';

  @override
  String get searchThisProject => 'Dieses Projekt durchsuchen';

  @override
  String get chatAboutThisProject => 'Über dieses Projekt chatten';

  @override
  String get addFiles => 'Dateien hinzufügen';

  @override
  String get writeANote => 'Notiz schreiben';

  @override
  String get addMeetings => 'Meetings hinzufügen';

  @override
  String get deleteProject => 'Projekt löschen';

  @override
  String get addThemHereOrFromA =>
      'Füge sie hier hinzu oder auf der Seite eines Meetings.';

  @override
  String get removeFromProject => 'Aus Projekt entfernen';

  @override
  String get filesNotesAndEMails => 'Dateien, Notizen und E-Mails';

  @override
  String get nothingAddedYet => 'Noch nichts hinzugefügt';

  @override
  String get addPicturesMarkdownPdfsEMails =>
      'Füge Bilder, Markdown, PDFs, E-Mails (.eml) oder Dokumente hinzu.';

  @override
  String get onItsWayFromAPaired => 'Unterwegs von einem gekoppelten Gerät';

  @override
  String get newNote => 'Neue Notiz';

  @override
  String get noteMarkdown => 'Notiz (Markdown)';

  @override
  String get everyMeetingIsAlreadyInA =>
      'Alle Meetings sind bereits in einem Projekt.';

  @override
  String get add => 'Hinzufügen';

  @override
  String get moveToProject => 'In Projekt verschieben';

  @override
  String get noProject => 'Kein Projekt';

  @override
  String get newProject => 'Neues Projekt…';

  @override
  String get newProject2 => 'Neues Projekt';

  @override
  String get create => 'Erstellen';

  @override
  String get theProjectCouldNotBeCreated =>
      'Das Projekt konnte nicht erstellt werden.';

  @override
  String get theMeetingCouldNotBeMoved =>
      'Das Meeting konnte nicht verschoben werden.';

  @override
  String get removedFromItsProject => 'Aus seinem Projekt entfernt';

  @override
  String movedTo(Object project) {
    return 'Nach $project verschoben';
  }

  @override
  String get searchProjects => 'Projekte durchsuchen';

  @override
  String get noProjectsYetAProjectGathers =>
      'Noch keine Projekte. Ein Projekt bündelt Meetings mit den Dateien, Notizen und E-Mails, die zu ihnen gehören.';

  @override
  String get theRecordingIsStillStartingTry =>
      'Die Aufnahme startet noch; versuche es erneut.';

  @override
  String recording(Object title) {
    return 'Aufnahme von „$title“';
  }

  @override
  String get recording2 => 'Aufnahme läuft';

  @override
  String paused(Object title) {
    return '„$title“ pausiert';
  }

  @override
  String get paused2 => 'Pausiert';

  @override
  String get finishingTranscription => 'Transkription wird abgeschlossen…';

  @override
  String get meetingTitle => 'Meeting-Titel';

  @override
  String get enterMeetingTitleOptional => 'Meeting-Titel eingeben (optional)';

  @override
  String get leaveEmptyToNameTheMeeting =>
      'Leer lassen, um das Meeting nach seiner Startzeit zu benennen';

  @override
  String get startingRecording => 'Aufnahme wird gestartet';

  @override
  String get startRecording => 'Aufnahme starten';

  @override
  String get pause => 'Pausieren';

  @override
  String get resume => 'Fortsetzen';

  @override
  String get stop => 'Beenden';

  @override
  String get language => 'Sprache';

  @override
  String get aChangeAppliesToWhatIs =>
      'Eine Änderung gilt für das, was als Nächstes gesagt wird';

  @override
  String get liveTranscript => 'Live-Transkript';

  @override
  String get theTranscriptAppearsHereAFew =>
      'Das Transkript erscheint hier einige Sekunden nach jeder Sprechpause.';

  @override
  String get nearbyDevicesSeeThisNameWhile =>
      'Geräte in der Nähe sehen diesen Namen, solange dieses Gerät sichtbar ist. Bereits gekoppelte Geräte behalten den Namen, den sie kennen.';

  @override
  String get meetingRenamed => 'Meeting umbenannt';

  @override
  String get thisTitleIsnTAllowed => 'Dieser Titel ist nicht erlaubt.';

  @override
  String get couldnTSaveTheNewTitle =>
      'Der neue Titel konnte nicht gespeichert werden. Versuche es erneut.';

  @override
  String get theNoteTakingEngineIsnT =>
      'Die Notiz-Engine ist gerade nicht verfügbar.';

  @override
  String get thisMeetingNoLongerExists2 =>
      'Dieses Meeting existiert nicht mehr.';

  @override
  String get couldnTRenameTheMeeting =>
      'Das Meeting konnte nicht umbenannt werden.';

  @override
  String get thisFileIsStillOnIts =>
      'Diese Datei ist noch unterwegs von einem gekoppelten Gerät.';

  @override
  String get thisPictureCannotBeShown =>
      'Dieses Bild kann nicht angezeigt werden.';

  @override
  String get thisKindOfDocumentCanT =>
      'Diese Art von Dokument kann in der App noch nicht angezeigt werden.';

  @override
  String get thisFileCannotBeOpened =>
      'Diese Datei kann nicht geöffnet werden.';

  @override
  String get thisFileCannotBeRead => 'Diese Datei kann nicht gelesen werden.';

  @override
  String get thisEMailCannotBeRead => 'Diese E-Mail kann nicht gelesen werden.';

  @override
  String get noSubject => '(kein Betreff)';

  @override
  String get from => 'Von';

  @override
  String get to => 'An';

  @override
  String get cc => 'Cc';

  @override
  String get sent => 'Gesendet';

  @override
  String attachments(Object attachmentNames) {
    return 'Anhänge: $attachmentNames';
  }

  @override
  String get theRestOfThisEMail =>
      'Der Rest dieser E-Mail ist zu lang zum Anzeigen.';

  @override
  String get searching => 'Suche läuft…';

  @override
  String get nothingFound => 'Nichts gefunden.';

  @override
  String get search2 => 'Suchen';

  @override
  String get pairYourDevicesAndSeeWhich =>
      'Kopple deine Geräte und sieh, welche verbunden sind.';

  @override
  String get transcriptionModel => 'Transkriptionsmodell';

  @override
  String get chooseTheWhisperModel => 'Wähle das Whisper-Modell.';

  @override
  String get languageModelsForMeetingSummariesAnd =>
      'Sprachmodelle für Meeting-Zusammenfassungen und den Chat, und ihre Prompts.';

  @override
  String get keepProjectsAndMeetingsAsFiles =>
      'Halte Projekte und Meetings als Dateien, die du mit jedem Editor bearbeiten kannst.';

  @override
  String get letYourOwnAssistantSuchAs =>
      'Lass deinen eigenen Assistenten, etwa Claude Code, deine Notizen lesen.';

  @override
  String get appLock => 'App-Sperre';

  @override
  String get defaultLanguage => 'Standardsprache';

  @override
  String get usedForNewRecordingsAutoDetects =>
      'Gilt für neue Aufnahmen. Auto erkennt die Sprache, während gesprochen wird.';

  @override
  String get transcribeOn => 'Transkribieren auf';

  @override
  String get anotherDeviceReceivesTheRecordingS =>
      'Ein anderes Gerät erhält das Audio der Aufnahme verschlüsselt und löscht es nach dem Transkribieren. Ist es nicht verfügbar, transkribiert dieses Gerät.';

  @override
  String get telemetry => 'Telemetrie';

  @override
  String get howLongTranscriptionTakesOnEach =>
      'Wie lange die Transkription auf jedem Gerät dauert und ihr geschätzter Energieverbrauch.';

  @override
  String get logLevel => 'Protokollstufe';

  @override
  String get howMuchDetailTheAppWrites =>
      'Wie ausführlich die App protokolliert. Debug und Trace helfen bei der Fehlersuche, schreiben aber mehr.';

  @override
  String get custom => 'Benutzerdefiniert';

  @override
  String get shareLogs => 'Protokolle teilen';

  @override
  String get theLast7DaysOfDiagnostic =>
      'Die Diagnoseprotokolle der letzten 7 Tage, etwa zum Anhängen an einen Fehlerbericht.';

  @override
  String get unableToReadTheAppLock =>
      'Die Einstellung der App-Sperre konnte nicht gelesen werden.';

  @override
  String get needsAndroid10OrLater => 'Erfordert Android 10 oder neuer.';

  @override
  String get setAScreenLockInAndroid =>
      'Richte in den Android-Einstellungen eine Displaysperre ein, um die App-Sperre zu nutzen.';

  @override
  String askForYourFingerprintFaceOr(Object lockAfterBackground) {
    return 'Nach Fingerabdruck, Gesicht oder Displaysperre fragen, wenn du nach $lockAfterBackground Sekunden Abwesenheit zu Koalino zurückkehrst. Aufnahme und Synchronisierung laufen weiter, während die App gesperrt ist.';
  }

  @override
  String get otherNetworks => 'Andere Netzwerke';

  @override
  String get aRelayOnYourOwnAlways =>
      'Ein Relay auf einem eigenen, stets eingeschalteten Gerät lässt deine Geräte einander auch außerhalb desselben WLANs erreichen. Die Synchronisierung im selben Netzwerk funktioniert auch ohne.';

  @override
  String get relayAddress => 'Relay-Adresse';

  @override
  String get deviceAddressesOptionalOnePerLine =>
      'Geräteadressen (optional, eine pro Zeile)';

  @override
  String get saveAddresses => 'Adressen speichern';

  @override
  String get nothingThatWasSharedCouldBe =>
      'Nichts von dem Geteilten konnte gelesen werden.';

  @override
  String couldNotBeRead(Object message, Object failedCount) {
    return '$message; $failedCount konnten nicht gelesen werden';
  }

  @override
  String saveAsTheSummaryOf(Object title) {
    return 'Als Zusammenfassung von „$title“ speichern';
  }

  @override
  String get addToAProjectInstead => 'Stattdessen zu einem Projekt hinzufügen';

  @override
  String get splitThisMeeting => 'Dieses Meeting trennen?';

  @override
  String itWasRecordedOnAndThe(Object devices, Object recordings) {
    return 'Es wurde auf $devices aufgenommen, und die App hat die Aufnahmen zusammengeführt. Beim Trennen erscheint die andere Aufnahme ($recordings) wieder eigenständig, mit eigenem Transkript, hier und auf deinen gekoppelten Geräten. Sie werden nicht erneut zusammengeführt. Dieses Meeting behält sein Transkript.';
  }

  @override
  String get unableToSplitThisMeeting =>
      'Dieses Meeting konnte nicht getrennt werden.';

  @override
  String get telemetryDashboard => 'Telemetrie-Dashboard';

  @override
  String get transcription => 'Transkription';

  @override
  String get latency => 'Latenz';

  @override
  String get tasks => 'Aufgaben';

  @override
  String get power => 'Energie';

  @override
  String get transcriptionDurationOverTime =>
      'Transkriptionsdauer im Zeitverlauf';

  @override
  String get latencyDistribution => 'Latenzverteilung';

  @override
  String get taskTypeBreakdown => 'Aufschlüsselung nach Aufgabentyp';

  @override
  String get powerConsumptionEstimate => 'Geschätzter Energieverbrauch';

  @override
  String get fastestLeastAccurate => 'Am schnellsten, am wenigsten genau';

  @override
  String get fastManyMistakesTheAppShips =>
      'Schnell, viele Fehler. Wird mit der App ausgeliefert';

  @override
  String get fastManyMistakes => 'Schnell, viele Fehler';

  @override
  String get goodBalanceOnMostComputers =>
      'Gute Balance auf den meisten Computern';

  @override
  String get accurateSlowerNeedsAbout2Gb =>
      'Genau, langsamer, braucht etwa 2 GB Arbeitsspeicher';

  @override
  String get olderNeedsAPowerfulComputer =>
      'Älter, braucht einen leistungsstarken Computer';

  @override
  String get mostAccurateNeedsAPowerfulComputer =>
      'Am genauesten, braucht einen leistungsstarken Computer';

  @override
  String get veryAccurateBestWithAGraphics =>
      'Sehr genau. Am besten mit einer Grafikkarte';

  @override
  String get thisModelMayBeTooLarge => 'Dieses Modell ist vielleicht zu groß';

  @override
  String mayNeedMoreMemoryThanThis(Object model) {
    return '$model braucht vielleicht mehr Arbeitsspeicher, als dieses Gerät hat, und Aufnahmen könnten fehlschlagen oder langsam laufen.';
  }

  @override
  String get downloadAnyway => 'Trotzdem herunterladen';

  @override
  String get transcriptionSettingsAreUnavailable =>
      'Die Transkriptionseinstellungen sind nicht verfügbar.';

  @override
  String get downloadFromQualcomm => 'Von Qualcomm herunterladen?';

  @override
  String get about360MbIsDownloadedFrom =>
      'Etwa 360 MB werden von Qualcomm heruntergeladen und belegen etwa 460 MB:\n\n• Qualcomms QNN-Laufzeit, mit der Apps die NPU nutzen können. Sie steht unter Qualcomms AI Stack License, nicht unter der Lizenz der App.\n• Whisper Small, von Qualcomm AI Hub für die NPU dieses Telefons kompiliert.\n\nJede Datei wird vor der Nutzung geprüft. Deine Aufnahmen verlassen nie das Telefon. Mit dem Download akzeptierst du die Bedingungen von Qualcomm.';

  @override
  String get acceptAndDownload => 'Akzeptieren und herunterladen';

  @override
  String get downloadingTheNpuFiles => 'NPU-Dateien werden heruntergeladen…';

  @override
  String get transcribeOnTheNpu => 'Auf der NPU transkribieren';

  @override
  String get thisPhoneHasAnAiChip =>
      'Dieses Telefon hat einen KI-Chip (NPU), der Whisper Small ausführen kann und den Prozessor frei lässt: 30 Sekunden Sprache dauern etwa 2 Sekunden. Dafür ist ein Download von Qualcomm nötig.';

  @override
  String get download => 'Herunterladen';

  @override
  String get runsWhisperSmallOnThePhone =>
      'Führt Whisper Small auf dem KI-Chip des Telefons aus, anstelle des unten gewählten Modells, auch für den Durchgang nach dem Meeting, sofern dieser kein eigenes Modell hat. Die Wortzeiten sind ungefähr.';

  @override
  String get removeTheNpuFiles => 'NPU-Dateien entfernen';

  @override
  String get alsoRecordTheComputerSSound =>
      'Auch den Ton des Computers aufnehmen';

  @override
  String get transcribesWhatTheComputerPlaysSuch =>
      'Transkribiert, was der Computer wiedergibt, etwa die andere Seite eines Anrufs, neben dem Mikrofon. Hört das Mikrofon auch die Lautsprecher, werden wiederholte Wörter nur einmal behalten.';

  @override
  String get improveTheTranscriptAfterTheMeeting =>
      'Transkript nach dem Meeting verbessern';

  @override
  String get transcribesEachMeetingAgainWithMore =>
      'Transkribiert jedes Meeting nach dem Ende erneut mit mehr Kontext; ein Telefon wartet, bis es lädt. Bis dahin bleibt die Sprache verschlüsselt, danach wird sie gelöscht.';

  @override
  String get modelForThisPass => 'Modell für diesen Durchgang';

  @override
  String get aLargerModelIsSlowerBut =>
      'Ein größeres Modell ist langsamer, aber genauer, und nach dem Meeting ist Zeit. Lade oben Modelle herunter.';

  @override
  String get sameAsForRecording => 'Wie bei der Aufnahme';

  @override
  String get keepSpeechForCombining => 'Sprache zum Kombinieren aufbewahren';

  @override
  String get whenAMeetingWasAlsoRecorded =>
      'Wurde ein Meeting auch auf einem anderen Gerät aufgenommen, wartet seine Sprache so lange auf das Kombinieren und wird dann gelöscht.';

  @override
  String get whisperModel => 'Whisper-Modell';

  @override
  String get biggerModelsMakeFewerMistakesAnd =>
      'Größere Modelle machen weniger Fehler und brauchen mehr Zeit und Speicher. Downloads kommen von Hugging Face. Diese Wahl gilt nur für dieses Gerät.';

  @override
  String automatic2(Object fileName, Object mode) {
    return 'Automatisch: $fileName$mode';
  }

  @override
  String usesUntilIsDownloadedTheBest(
      Object fileName, Object waiting, Object reason) {
    return 'Verwendet $fileName, bis $waiting heruntergeladen ist, das für dieses Gerät am besten passt. $reason';
  }

  @override
  String picksTheBestModelForThis(Object fileName, Object mode) {
    return 'Wählt das beste Modell für dieses Gerät: $fileName$mode.';
  }

  @override
  String download2(Object sizeBytes) {
    return '$sizeBytes herunterladen';
  }

  @override
  String tryLabel(Object model) {
    return '$model testen';
  }

  @override
  String get use => 'Verwenden';

  @override
  String suggestedForThisDevice(Object model, Object mode) {
    return 'Empfohlen für dieses Gerät: $model$mode';
  }

  @override
  String get efficientMode => 'Effizienter Modus';

  @override
  String runsTheSameModelsOnThe(Object gpuName) {
    return 'Führt dieselben Modelle auf dem Grafikchip ($gpuName) aus, meist schneller. Aus nutzt wie üblich den Prozessor.';
  }

  @override
  String get theGraphicsChipStoppedTheApp =>
      'Der Grafikchip hat die App beim letzten Mal angehalten, daher transkribiert dieses Gerät mit dem Prozessor.';

  @override
  String get tryTheGraphicsChipAgain => 'Grafikchip erneut versuchen';

  @override
  String get onThisDevice => 'auf diesem Gerät';

  @override
  String get usedByAutomatic => 'von Automatisch verwendet';

  @override
  String get englishOnly => 'Nur Englisch';

  @override
  String get mayBeTooLargeForThis => 'vielleicht zu groß für dieses Gerät';

  @override
  String get doesnTMatchYourLanguage => 'passt nicht zu deiner Sprache';

  @override
  String get notEnoughFreeSpace => 'nicht genug freier Speicher';

  @override
  String get cancelDownload => 'Download abbrechen';

  @override
  String get glossary => 'Glossar';

  @override
  String get namesProjectNamesAndTermsOne =>
      'Namen, Projektnamen und Begriffe, einer pro Zeile. Die Transkription schreibt sie wie hier.';

  @override
  String get saveGlossary => 'Glossar speichern';

  @override
  String get voiceGallery => 'Stimmengalerie';

  @override
  String get tryAgain => 'Erneut versuchen';

  @override
  String get unknownVoices => 'Unbekannte Stimmen';

  @override
  String get noUnknownSpeakersToReview => 'Keine unbekannten Stimmen zu prüfen';

  @override
  String get savedPeople => 'Gespeicherte Personen';

  @override
  String get noSavedPeopleYetNameA =>
      'Noch keine gespeicherten Personen. Benenne eine Stimme, um jemanden hinzuzufügen.';

  @override
  String delete4(Object displayName) {
    return '$displayName löschen';
  }

  @override
  String get playing => 'Wird abgespielt…';

  @override
  String get untitledMeeting => 'Meeting ohne Titel';

  @override
  String get play => 'Abspielen';

  @override
  String get deleteThisVoice => 'Diese Stimme löschen';

  @override
  String theCallShowedSpeakingCheckThen(Object suggestedName) {
    return 'Der Anruf zeigte $suggestedName beim Sprechen. Prüfe es, dann Zuordnen.';
  }

  @override
  String get chooseASavedPerson => 'Gespeicherte Person wählen';

  @override
  String get assign => 'Zuordnen';

  @override
  String get deleteTheseVoices => 'Diese Stimmen löschen';

  @override
  String get theseSoundLikeOnePersonUntick =>
      'Diese klingen nach einer Person. Entferne den Haken bei allen, die jemand anderes sind.';

  @override
  String itsVoiceSamplesGoForGood(Object voices) {
    return 'Ihre $voices Stimmproben werden endgültig gelöscht, hier und auf deinen gekoppelten Geräten. Die Transkripte bleiben.';
  }

  @override
  String get itsVoiceSampleGoesForGood =>
      'Ihre Stimmprobe wird endgültig gelöscht, hier und auf deinen gekoppelten Geräten. Das Transkript bleibt.';

  @override
  String get theirNameAndVoiceSamplesGo =>
      'Name und Stimmproben werden endgültig gelöscht, hier und auf deinen gekoppelten Geräten. Transkripte bleiben und nummerieren die Person als sprechende Person.';

  @override
  String unknownSpeaker(Object index) {
    return 'Unbekannte Stimme $index';
  }

  @override
  String attachedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Dateien angehängt',
      one: '1 Datei angehängt',
    );
    return '$_temp0';
  }

  @override
  String addedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Dateien hinzugefügt',
      one: '1 Datei hinzugefügt',
    );
    return '$_temp0';
  }

  @override
  String emailAttachmentsLeft(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count E-Mail-Anhänge blieben in der E-Mail',
      one: '1 E-Mail-Anhang blieb in der E-Mail',
    );
    return '$_temp0';
  }

  @override
  String addedMeetings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Meetings hinzugefügt',
      one: '1 Meeting hinzugefügt',
    );
    return '$_temp0';
  }

  @override
  String otherRecordings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Aufnahmen',
      one: '1 Aufnahme',
    );
    return '$_temp0';
  }

  @override
  String get severalDevices => 'mehreren Geräten';

  @override
  String meetingCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Meetings',
      one: '1 Meeting',
    );
    return '$_temp0';
  }

  @override
  String fileCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Dateien',
      one: '1 Datei',
    );
    return '$_temp0';
  }

  @override
  String chunkCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Abschnitte',
      one: '1 Abschnitt',
    );
    return '$_temp0';
  }

  @override
  String resultCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Ergebnisse',
      one: '1 Ergebnis',
    );
    return '$_temp0';
  }

  @override
  String get moreNotShown =>
      ', weitere nicht angezeigt: füge Wörter hinzu, um einzugrenzen';

  @override
  String cpuCores(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count CPU-Kerne',
      one: '1 CPU-Kern',
    );
    return '$_temp0';
  }

  @override
  String dayCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Tage',
      one: '1 Tag',
    );
    return '$_temp0';
  }

  @override
  String voiceSampleCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Stimmproben',
      one: '1 Stimmprobe',
      zero: 'noch keine Stimmprobe, daher nicht an der Stimme erkannt',
    );
    return '$_temp0';
  }

  @override
  String picturesIncluded(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Bilder wurden einbezogen',
      one: '1 Bild wurde einbezogen',
    );
    return '$_temp0';
  }

  @override
  String partCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Teile',
      one: '1 Teil',
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
      other: '$count Stimmen $name zugeordnet',
      one: '$name zugeordnet',
    );
    return '$_temp0';
  }

  @override
  String addSharedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count geteilte Dateien zu einem Projekt hinzufügen',
      one: 'Geteilte Datei zu einem Projekt hinzufügen',
    );
    return '$_temp0';
  }

  @override
  String keepSpeechSaved(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other:
          'Gespeichert. Sprache wartet jetzt bis zu $count Tage auf das Kombinieren.',
      one:
          'Gespeichert. Sprache wartet jetzt bis zu 1 Tag auf das Kombinieren.',
    );
    return '$_temp0';
  }

  @override
  String titlesCanBeAtMost(Object max) {
    return 'Titel dürfen höchstens $max Zeichen haben.';
  }

  @override
  String promptUnknownPlaceholder(Object example) {
    return 'Der Prompt enthält einen unbekannten oder nicht geschlossenen Platzhalter $example.';
  }

  @override
  String get live => '● Live';

  @override
  String get offline => '○ Offline';

  @override
  String minutesShort(Object minutes) {
    return '$minutes Min.';
  }

  @override
  String get appLanguage => 'App-Sprache';

  @override
  String get appLanguageDescription =>
      'Die Sprache der Menüs und Meldungen von Koalino.';

  @override
  String get systemLanguage => 'Systemsprache';

  @override
  String photosAddedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Fotos hinzugefügt',
      one: '1 Foto hinzugefügt',
    );
    return '$_temp0';
  }

  @override
  String moreModels(int count) {
    return 'Weitere Modelle ($count)';
  }

  @override
  String get moreModelsHint =>
      'Kleinere Versionen mit etwas mehr Fehlern, reine Englisch-Versionen und Modelle, die für dieses Gerät zu groß sein können.';

  @override
  String get microphoneSilencedByCall =>
      'Android schaltet das Mikrofon stumm, während eine andere App es für einen Anruf nutzt: Bis der Anruf endet, wird nichts aufgenommen.';

  @override
  String get handoffInTheBrowser =>
      'Koalino öffnet die Website in deinem Browser, wo du mit deinem eigenen Konto angemeldet bist, und übergibt ihr die Besprechung mit deinem Prompt. Das nächste Fenster zeigt, was dort zu tun ist.';

  @override
  String toKeepTheSummaryCopyTheWebsite(String appName) {
    return 'Um die Zusammenfassung zu behalten, kopiere die Antwort von $appName auf der Website und kehre zu Koalino zurück.';
  }

  @override
  String get useYourClaudeOrChatgptPlan =>
      'Deinen Claude- oder ChatGPT-Plan nutzen';

  @override
  String get summariesCanUseYourOwnPlan =>
      'Zusammenfassungen können dein eigenes Claude- oder ChatGPT-Abo nutzen, ohne API-Schlüssel. Installiere Claude Code oder Codex und melde dich einmal an: Danach schreibt Koalino Zusammenfassungen selbst.';

  @override
  String get installClaudeCode => 'Claude Code installieren';

  @override
  String get forAClaudePlanOpensTheGuide =>
      'Für einen Claude-Plan. Öffnet die Installationsanleitung.';

  @override
  String get installCodex => 'Codex installieren';

  @override
  String get forAChatgptPlanOpensTheGuide =>
      'Für einen ChatGPT-Plan. Öffnet die Installationsanleitung.';

  @override
  String get skipUseTheWebsite => 'Überspringen: Website nutzen';

  @override
  String get koalinoCopiesTheMeetingAndOpens =>
      'Nichts zu installieren: Koalino kopiert das Meeting und öffnet claude.ai oder chatgpt.com, und du fügst es dort ein.';

  @override
  String get addAConnection => 'Verbindung hinzufügen';

  @override
  String get claudeCodeOrCodexAlreadyInstalled =>
      'Claude Code oder Codex bereits installiert, oder ein Server in deinem Netzwerk.';

  @override
  String get noConnectionNeededUseTheWebsite =>
      'Keine Verbindung nötig. Öffne in einem Meeting Zusammenfassung und wähle Eigene KI-App verwenden…: Koalino kopiert das Meeting und öffnet claude.ai oder chatgpt.com.';

  @override
  String get afterInstallingAddTheConnection =>
      'Nach der Installation und Anmeldung komm zurück und wähle Verbindung hinzufügen → Verbindung hinzufügen → Claude Code oder Codex.';

  @override
  String get claudeAiOrChatgptComInYourBrowser =>
      'claude.ai oder chatgpt.com im Browser, mit deinem Konto';

  @override
  String get newMeeting => 'Neue Besprechung';

  @override
  String get newMeetingExplanation =>
      'Eine Besprechung ohne Aufnahme. Schreib Notizen oder füge Dateien hinzu: Zusammenfassungen und Chat verwenden sie.';

  @override
  String get titleOptional => 'Titel (optional)';

  @override
  String handoffStepPressEnter(String site) {
    return '$site öffnet sich mit der Besprechung bereits im Nachrichtenfeld. Drück die Eingabetaste, um sie zu senden.';
  }

  @override
  String handoffStepSentByItself(String site) {
    return '$site öffnet sich und sendet die Besprechung von selbst. Melde dich zuerst an, falls es danach fragt.';
  }

  @override
  String handoffStepPaste(String site) {
    return '$site öffnet sich und Koalino kopiert die Besprechung. Klick ins Nachrichtenfeld, füge sie ein (Strg+V) und drück die Eingabetaste.';
  }

  @override
  String get handoffStepDragFiles =>
      'Ein Ordner mit den Dateien öffnet sich ebenfalls: Zieh sie alle in das Nachrichtenfeld, damit sie als Dateien mitgehen.';

  @override
  String get handoffStepCopyAnswer =>
      'Wenn die Antwort fertig ist, drück die Kopieren-Schaltfläche darunter.';

  @override
  String get handoffStepComeBack =>
      'Komm zurück zu Koalino: Die Antwort wird zur Zusammenfassung der Besprechung, mit Rückgängig.';

  @override
  String get handoffStepAskThere =>
      'Stell dann dort deine Fragen zur Besprechung.';

  @override
  String openSite(String site) {
    return '$site öffnen';
  }

  @override
  String writtenBySourceOn(String source, String createdAt) {
    return 'Geschrieben von $source am $createdAt';
  }

  @override
  String get noRecordingWriteNotesOrAttach =>
      'Keine Aufnahme. Schreib Notizen oder hänge Dateien an: Zusammenfassungen und Chat verwenden sie.';

  @override
  String handoffStepOpensFilled(String site) {
    return '$site öffnet sich mit der Besprechung bereits im Nachrichtenfeld.';
  }

  @override
  String handoffStepOpensCopied(String site) {
    return '$site öffnet sich, und Koalino kopiert die Besprechung.';
  }

  @override
  String get handoffStepSend => 'Drück die Eingabetaste, um sie zu senden.';

  @override
  String get handoffStepPasteSend =>
      'Klick ins Nachrichtenfeld, füge die Besprechung ein (Strg+V) und drück die Eingabetaste.';

  @override
  String get chatHandoffIntro =>
      'Stelle deine Fragen in deiner eigenen KI-App, mit deinem eigenen Konto. Koalino übergibt ihr Notizen und Transkripte, Fotos und Dateien gehen als Dateien mit.';

  @override
  String get plainTextFile => 'Reiner Text (.txt)';

  @override
  String get unableToExportTheSummary =>
      'Die Zusammenfassung konnte nicht exportiert werden';

  @override
  String get addAVoice => 'Stimme hinzufügen';

  @override
  String alreadySavedVoiceAddsToThem(String name) {
    return '$name ist bereits gespeichert: Diese Stimme wird hinzugefügt.';
  }

  @override
  String addVoicesTo(String name) {
    return 'Stimmen zu $name hinzufügen';
  }

  @override
  String tickTheVoicesThatAre(String name) {
    return 'Diese Stimmen wurden niemandem zugeordnet. Hake die an, die $name gehören; mit Abspielen prüfen.';
  }

  @override
  String addTo(String name) {
    return 'Zu $name hinzufügen';
  }

  @override
  String get apiKey => 'API-Schlüssel';

  @override
  String get onlyForOnlineServicesKeptIn =>
      'Nur für Online-Dienste. Im Schlüsselbund dieses Geräts gespeichert, nur an diese Adresse gesendet.';

  @override
  String get aKeyIsSavedLeaveEmpty =>
      'Ein Schlüssel ist gespeichert. Leer lassen, um ihn zu behalten.';

  @override
  String get removeTheSavedKey => 'Gespeicherten Schlüssel entfernen';

  @override
  String get presetOllama =>
      'Auf deinem Computer oder im Netzwerk: kostenlos, privat, offline nutzbar (Gemma, Qwen, Llama…).';

  @override
  String get presetLmStudio =>
      'Wie Ollama, mit einer Desktop-App zur Modellwahl.';

  @override
  String get presetGemini =>
      'Google. Kostenlose Stufe mit Limits; dabei darf Google Gesendetes zur Produktverbesserung nutzen. Schlüssel: aistudio.google.com.';

  @override
  String get presetDeepseek =>
      'Nutzungsbasiert, sehr günstig, Prepaid-Guthaben. Schlüssel: platform.deepseek.com.';

  @override
  String get presetQwen =>
      'Alibaba. Nutzungsbasiert. Schlüssel: Alibaba Cloud Model Studio.';

  @override
  String get presetKimi =>
      'Moonshot AI. Nutzungsbasiert. Schlüssel: platform.moonshot.ai.';

  @override
  String get presetMistral =>
      'Mistral (Frankreich). Kostenlose Stufe mit engen Limits. Schlüssel: console.mistral.ai.';

  @override
  String get presetOpenrouter =>
      'Ein Schlüssel für viele Modelle (Gemini, DeepSeek, Qwen…), manche kostenlos; der Text geht auch über OpenRouter. Schlüssel: openrouter.ai.';

  @override
  String get keepMeetingRecordings => 'Besprechungsaufnahmen behalten';

  @override
  String get keepMeetingRecordingsSubtitle =>
      'Den Ton jeder Besprechung nach der Transkription verschlüsselt auf diesem Gerät behalten, um ihn abzuspielen. Standardmäßig aus.';

  @override
  String get keepMeetingRecordingsQuestion => 'Besprechungsaufnahmen behalten?';

  @override
  String get keepRecordingsWarning =>
      'Aufnahmen enthalten die Stimmen aller Teilnehmenden. Sag den Personen, dass du aufnimmst, und beachte die Einwilligungsregeln vor Ort: Vielerorts braucht das Aufnehmen anderer deren Erlaubnis. Aufnahmen bleiben verschlüsselt auf diesem Gerät und werden nicht synchronisiert; sie belegen etwa 2 MB pro 10 Minuten Sprache. Sie werden mit ihrer Besprechung gelöscht oder alle zusammen mit „Alle Aufnahmen löschen“. Nur Sprache wird behalten, stille Stellen werden als Stille abgespielt.';

  @override
  String get keepRecordings => 'Aufnahmen behalten';

  @override
  String recordingsKept(int count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Aufnahmen behalten · $size MB',
      one: '1 Aufnahme behalten · $size MB',
      zero: 'Keine Aufnahmen behalten',
    );
    return '$_temp0';
  }

  @override
  String get wipeAllRecordings => 'Alle Aufnahmen löschen';

  @override
  String get wipeAllRecordingsQuestion => 'Alle Aufnahmen löschen?';

  @override
  String get wipeAllRecordingsExplanation =>
      'Jede behaltene Besprechungsaufnahme auf diesem Gerät wird gelöscht. Transkripte und Notizen bleiben. Das kann nicht rückgängig gemacht werden.';

  @override
  String recordingsWiped(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Aufnahmen gelöscht.',
      one: '1 Aufnahme gelöscht.',
      zero: 'Keine Aufnahmen zu löschen.',
    );
    return '$_temp0';
  }

  @override
  String get recordingsUnavailable => 'Aufnahmen sind gerade nicht erreichbar.';

  @override
  String get playRecording => 'Aufnahme abspielen';

  @override
  String get stopRecordingPlayback => 'Wiedergabe stoppen';

  @override
  String get recordingCouldNotBePlayed =>
      'Die Aufnahme konnte nicht abgespielt werden.';

  @override
  String get columnWidth => 'Spaltenbreite';

  @override
  String get subprojects => 'Unterprojekte';

  @override
  String get newSubproject => 'Neues Unterprojekt';

  @override
  String get moveProjectTo => 'Verschieben nach…';

  @override
  String get topLevel => 'Oberste Ebene';

  @override
  String moveProjectTitle(String project) {
    return '$project verschieben nach';
  }

  @override
  String get movedToTopLevel => 'Auf die oberste Ebene verschoben';

  @override
  String get theProjectCouldNotBeMoved =>
      'Das Projekt konnte nicht verschoben werden.';

  @override
  String subprojectCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count Unterprojekte',
      one: '1 Unterprojekt',
    );
    return '$_temp0';
  }

  @override
  String get chooseAProject =>
      'Wähle ein Projekt, um seine Besprechungen und Dateien zu sehen.';

  @override
  String get chooseAMeetingOrFile =>
      'Wähle eine Besprechung oder eine Datei, um sie hier zu sehen.';

  @override
  String get backgroundPassTitle =>
      'Transkripte auch bei geschlossenem Koalino verbessern';

  @override
  String get backgroundPassAllowed =>
      'Erlaubt: Das Telefon verbessert wartende Transkripte, sobald es lädt, auch wenn Koalino geschlossen oder der Energiesparmodus aktiv ist.';

  @override
  String get backgroundPassAsk =>
      'Tippen, damit Android dies beim Laden erledigt, auch im Energiesparmodus. Sonst geschieht es nur bei geöffnetem Koalino.';

  @override
  String get chatPrompt => 'Chat-Prompt';

  @override
  String chatPromptHelp(String placeholder) {
    return '$placeholder steht für das, wonach du fragst: eine Besprechung, ein Projekt oder alle deine Notizen. Antworten nennen ihre Quellen immer mit einer Nummer, und deine Notizen folgen auf den Prompt.';
  }

  @override
  String get saveChatPrompt => 'Chat-Prompt speichern';

  @override
  String get chatPromptSaved => 'Chat-Prompt gespeichert.';

  @override
  String get theBuiltInChatPromptIsBack =>
      'Der eingebaute Chat-Prompt ist zurück.';

  @override
  String get promptsSharedWithPairedDevices =>
      'Beide Prompts sind auf all deinen gekoppelten Geräten gleich: Eine Änderung hier erreicht sie, sobald sie sich verbinden.';

  @override
  String get deviceOnline => 'Online';

  @override
  String get deviceOffline => 'Offline';

  @override
  String get neverSynced => 'Noch nie synchronisiert';

  @override
  String get syncedJustNow => 'Gerade synchronisiert';

  @override
  String lastSyncedMinutesAgo(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Zuletzt vor $count Minuten synchronisiert',
      one: 'Zuletzt vor 1 Minute synchronisiert',
    );
    return '$_temp0';
  }

  @override
  String lastSyncedHoursAgo(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Zuletzt vor $count Stunden synchronisiert',
      one: 'Zuletzt vor 1 Stunde synchronisiert',
    );
    return '$_temp0';
  }

  @override
  String lastSyncedOn(String date) {
    return 'Zuletzt synchronisiert: $date';
  }

  @override
  String devicesOnlineCount(int online, int total) {
    return '$online von $total online';
  }

  @override
  String get microphoneSeemsMutedOrOff =>
      'Das Mikrofon scheint stummgeschaltet oder aus zu sein: Es ist nichts zu hören. Prüfe den Stummschalter oder die Taste und ob der Mikrofonzugriff in den Systemeinstellungen an ist.';

  @override
  String get windowsBlocksTheMicrophone =>
      'Windows hält Apps vom Mikrofon fern: Es ist nichts zu hören. Schalte unter Einstellungen → Datenschutz und Sicherheit → Mikrofon den Mikrofonzugriff und „Desktop-Apps den Zugriff auf Ihr Mikrofon erlauben“ ein.';

  @override
  String get backUpAllData => 'Alle Daten sichern';

  @override
  String get backUpAllDataDescription =>
      'Besprechungen, Notizen, Anhänge und behaltene Aufnahmen in einer verschlüsselten Datei speichern';

  @override
  String get restoreFromBackup => 'Aus Sicherung wiederherstellen';

  @override
  String get restoreFromBackupDescription =>
      'Die Daten auf diesem Gerät durch eine Sicherung ersetzen';

  @override
  String get backupPassphraseTitle => 'Passphrase der Sicherung';

  @override
  String get backupPassphraseExplanation =>
      'Die Sicherung wird mit dieser Passphrase verschlüsselt. Ohne sie kann niemand die Sicherung öffnen, auch du nicht. Bewahre sie sicher auf.';

  @override
  String get backupPassphraseLabel => 'Passphrase';

  @override
  String get repeatPassphraseLabel => 'Passphrase wiederholen';

  @override
  String get passphraseTooShort => 'Verwende mindestens 8 Zeichen.';

  @override
  String get passphrasesDoNotMatch => 'Die Passphrasen stimmen nicht überein.';

  @override
  String get continueAction => 'Weiter';

  @override
  String get backingUp => 'Sicherung läuft…';

  @override
  String get restoringBackup => 'Wiederherstellung läuft…';

  @override
  String backupSaved(int files, String size) {
    return 'Sicherung gespeichert: $files Dateien, $size MB.';
  }

  @override
  String get restoreConfirmTitle => 'Daten auf diesem Gerät ersetzen?';

  @override
  String get restoreConfirmBody =>
      'Die Besprechungen und Notizen auf diesem Gerät werden durch die der Sicherung ersetzt. Die aktuellen Daten werden bis zur nächsten Wiederherstellung aufbewahrt. Danach startet Koalino neu.';

  @override
  String get restoreAction => 'Wiederherstellen';

  @override
  String restoreReady(int files) {
    return '$files Dateien wiederhergestellt. Koalino startet jetzt neu, um sie zu öffnen.';
  }

  @override
  String get restartNow => 'Neu starten';

  @override
  String get backupWrongPassphrase => 'Die Passphrase ist falsch.';

  @override
  String get backupNotABackup => 'Diese Datei ist keine Koalino-Sicherung.';

  @override
  String get backupTooNew =>
      'Diese Sicherung stammt von einer neueren Koalino-Version. Aktualisiere zuerst die App.';

  @override
  String get backupDamaged =>
      'Die Sicherung ist beschädigt oder unvollständig.';

  @override
  String get backupFailed =>
      'Der Vorgang konnte nicht abgeschlossen werden. Prüfe den freien Speicher und versuche es erneut.';

  @override
  String get automaticBackups => 'Automatische Sicherungen';

  @override
  String get automaticBackupsDescription =>
      'Täglich während des Ladens in einen Ordner deiner Wahl, auch in der Cloud (Google Drive, Nextcloud, OneDrive…). Verschlüsselt, bevor es das Telefon verlässt.';

  @override
  String automaticBackupFolder(String folder) {
    return 'Ordner: $folder';
  }

  @override
  String get noAutomaticBackupYet => 'Noch keine automatische Sicherung.';

  @override
  String lastAutomaticBackup(String time) {
    return 'Letzte Sicherung: $time';
  }

  @override
  String lastAutomaticBackupFailed(String time) {
    return 'Die Sicherung vom $time ist fehlgeschlagen. Prüfe den Ordner.';
  }

  @override
  String get backUpNow => 'Jetzt sichern';

  @override
  String get backupFolderUnavailable =>
      'In den Sicherungsordner kann nicht geschrieben werden. Prüfe, ob er noch existiert und seine App angemeldet ist, oder wähle einen anderen Ordner.';

  @override
  String get automaticBackupsNotSetUp =>
      'Automatische Sicherungen müssen neu eingerichtet werden: aus- und wieder einschalten.';

  @override
  String get renameSpeaker => 'Sprecher umbenennen';

  @override
  String get nameThisVoice => 'Diese Stimme benennen';

  @override
  String get doubleClickOrLongPressToRename =>
      'Doppelklicken oder lange drücken zum Umbenennen';

  @override
  String get theNewNameAppliesToEveryPassage =>
      'Der neue Name gilt für jede Passage dieser Stimme, in allen deinen Besprechungen und auf deinen gekoppelten Geräten.';

  @override
  String anotherSavedPersonIsCalled(Object name) {
    return 'Eine andere gespeicherte Person heißt bereits „$name“. Wenn es dieselbe Person ist, führe beide zusammen.';
  }

  @override
  String get useOneTo100CharactersOnOneLine =>
      'Verwende 1 bis 100 Zeichen in einer Zeile.';

  @override
  String versionNumber(Object version) {
    return 'Version $version';
  }

  @override
  String get developerSettings => 'Entwicklereinstellungen';

  @override
  String get appVariablesForExpertsAndDevelopers =>
      'App-Variablen für Fachleute und Entwickler';

  @override
  String tapsLeftToTurnOnDeveloperSettings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other:
          'Noch $count-mal tippen, um die Entwicklereinstellungen einzuschalten',
      one: 'Noch 1 Tippen, um die Entwicklereinstellungen einzuschalten',
    );
    return '$_temp0';
  }

  @override
  String get developerSettingsAreOn =>
      'Die Entwicklereinstellungen sind eingeschaltet';

  @override
  String get developerSettingsWarning =>
      'Diese Variablen ändern, wie Koalino intern arbeitet. Ein falscher Wert kann Transkription, Synchronisierung oder die Browsererweiterung stoppen. Gespeicherte Werte gelten nach einem Neustart von Koalino.';

  @override
  String get restartKoalinoToApply =>
      'Starte Koalino neu, um die gespeicherten Werte anzuwenden.';

  @override
  String nowAndDefault(Object now, Object defaultValue) {
    return 'Jetzt: $now · Standard: $defaultValue';
  }

  @override
  String get notSet => 'nicht gesetzt';

  @override
  String savedForTheNextStart(Object value) {
    return 'Für den nächsten Start gespeichert: $value';
  }

  @override
  String get useDefault => 'Standard verwenden';

  @override
  String get resetAll => 'Alle zurücksetzen';

  @override
  String get turnOffDeveloperSettings => 'Entwicklereinstellungen ausschalten';

  @override
  String get turnOffDeveloperSettingsExplanation =>
      'Jeder Wert kehrt beim nächsten Start zum Standard zurück, und dieser Bereich wird wieder ausgeblendet.';

  @override
  String get turnOff => 'Ausschalten';

  @override
  String mergeIntoAnotherPerson(Object name) {
    return '$name mit einer anderen Person zusammenführen';
  }

  @override
  String mergeNameInto(Object name) {
    return '$name zusammenführen mit…';
  }

  @override
  String mergeWarning(Object source, Object target) {
    return 'Alle Passagen und Stimmproben von $source gehen an $target, und $source wird gelöscht, auch auf deinen gekoppelten Geräten. Das kann nicht rückgängig gemacht werden.';
  }

  @override
  String get merge => 'Zusammenführen';

  @override
  String mergeWithName(Object name) {
    return 'Mit $name zusammenführen';
  }

  @override
  String mergedInto(Object source, Object target) {
    return '$source gehört jetzt zu $target';
  }

  @override
  String get thesePeopleCouldNotBeMerged =>
      'Diese Personen konnten nicht zusammengeführt werden. Versuche es erneut.';

  @override
  String get peopleCannotBeMergedWhileRecording =>
      'Personen können nicht zusammengeführt werden, während eine Besprechung aufgenommen wird.';

  @override
  String get notEnoughFreeSpaceToDownload =>
      'Für den Download ist noch nicht genug freier Speicher vorhanden.';

  @override
  String get downloadTheBestModelByItself =>
      'Bestes Modell selbst herunterladen';

  @override
  String get downloadTheBestModelByItselfHint =>
      'Im WLAN und bei genug Speicher wird das Modell, das Automatisch für dieses Gerät wählt, beim Start heruntergeladen. Aufnahmen nutzen bis dahin das aktuelle Modell.';

  @override
  String gettingAMoreAccurateModel(String name, String percent) {
    return 'Ein genaueres Modell ($name) für dieses Gerät wird geladen: $percent';
  }

  @override
  String get recordingsUseTheCurrentModelMeanwhile =>
      'Aufnahmen nutzen weiter das aktuelle Modell, bis es bereit ist.';

  @override
  String get youCanRecordOnceItIsReady =>
      'Du kannst aufnehmen, sobald es bereit ist.';

  @override
  String get npuUsedAtTheLastRecording => 'Bei der letzten Aufnahme verwendet.';

  @override
  String get npuFailedAtTheLastRecording =>
      'Die NPU dieses Telefons hat die Prüfung nicht bestanden, daher transkribiert der Prozessor. Entferne die NPU-Dateien und lade sie erneut herunter, um es noch einmal zu versuchen.';

  @override
  String get skipBack10Seconds => '10 Sekunden zurück';

  @override
  String get skipForward10Seconds => '10 Sekunden vor';

  @override
  String get playbackSpeed => 'Wiedergabegeschwindigkeit';

  @override
  String get followTranscript => 'Im Transkript mitlesen';

  @override
  String get playFromHere => 'Ab hier abspielen';

  @override
  String get openingTheRecording => 'Aufnahme wird geöffnet…';

  @override
  String get recordingPlayer => 'Aufnahme-Player';

  @override
  String get shareTheRecording => 'Aufnahme teilen';

  @override
  String get saveTheRecording => 'Aufnahme speichern';

  @override
  String get unableToShareTheRecording =>
      'Die Aufnahme kann nicht geteilt werden';

  @override
  String get unableToSaveTheRecording =>
      'Die Aufnahme kann nicht gespeichert werden';

  @override
  String todayAt(String time) {
    return 'Heute $time';
  }

  @override
  String yesterdayAt(String time) {
    return 'Gestern $time';
  }

  @override
  String get unnamedDevice => 'Unbenanntes Gerät';

  @override
  String get checkTheMicrophone =>
      'Prüfe, ob ein Mikrofon angeschlossen ist und Koalino es verwenden darf.';

  @override
  String get speechModelCouldNotLoad =>
      'Die Transkription konnte nicht starten: Das Sprachmodell fehlt oder ließ sich nicht laden. Wähle oder lade ein Modell.';

  @override
  String get processorCannotTranscribe =>
      'Der Prozessor dieses Geräts kann nicht transkribieren. In den Transkriptionseinstellungen kann ein anderes deiner Geräte für dieses transkribieren.';

  @override
  String get secureStorageLocked =>
      'Der sichere Speicher von Koalino ist gesperrt. Entsperre ihn und versuche es erneut.';

  @override
  String get startKeepsFailing =>
      'Versuche es erneut. Wenn es weiter fehlschlägt, starte Koalino neu.';

  @override
  String get openTranscriptionSettings => 'Transkriptionseinstellungen öffnen';

  @override
  String get people => 'Personen';

  @override
  String get recordingNow => 'Aufnahme läuft';

  @override
  String get pausedNow => 'Pausiert';

  @override
  String get recordAMeeting => 'Besprechung aufnehmen';

  @override
  String get voicesAppearAfterRecording =>
      'Nach einer Aufnahme erscheinen hier Stimmen, die Koalino nicht erkennt, damit du sie benennen kannst.';

  @override
  String get askAi => 'KI fragen';

  @override
  String get shareAction => 'Teilen';

  @override
  String get moreActions => 'Weitere Aktionen';

  @override
  String get files => 'Dateien';

  @override
  String get noFilesYet =>
      'Noch keine Fotos oder Dateien. Fotos, die du während einer Besprechung aufnimmst, und angehängte Dateien erscheinen hier.';

  @override
  String get summariesNeedAnAi =>
      'Zusammenfassungen schreibt eine KI, die du verbindest: ein Modell auf diesem Gerät, dein Claude- oder ChatGPT-Abo oder ein Onlinedienst.';

  @override
  String get addAnAiConnection => 'KI-Verbindung hinzufügen';
}
