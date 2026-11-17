// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Italian (`it`).
class AppLocalizationsIt extends AppLocalizations {
  AppLocalizationsIt([String locale = 'it']) : super(locale);

  @override
  String get unableToCheckTheAppLock =>
      'Impossibile verificare il blocco dell\'app. Riprova.';

  @override
  String get unableToShowTheUnlockPrompt =>
      'Impossibile mostrare la richiesta di sblocco. Riprova.';

  @override
  String get setAScreenLockOnThis =>
      'Imposta prima un blocco schermo su questo dispositivo.';

  @override
  String get unableToChangeTheAppLock =>
      'Impossibile modificare il blocco dell\'app. Riprova.';

  @override
  String get aCallApp => 'Un\'app di chiamate';

  @override
  String get call => 'Chiamata';

  @override
  String call2(Object app) {
    return 'Chiamata $app';
  }

  @override
  String theCallEndedSoTheRecording(Object tiedApp) {
    return 'La chiamata $tiedApp è terminata, quindi la registrazione si è fermata.';
  }

  @override
  String get restartYourBrowserAndCallApps =>
      'Riavvia una volta il browser e le app di chiamata, così Koalino può leggere chi sta parlando nelle chiamate.';

  @override
  String get unableToLoadDevices => 'Impossibile caricare i dispositivi';

  @override
  String get thisDeviceIsNotOnA =>
      'Questo dispositivo non è ancora in rete. Collegalo e riprova.';

  @override
  String get thatDidNotWorkTryAgain => 'Non ha funzionato. Riprova.';

  @override
  String get use1To64CharactersWithout =>
      'Usa da 1 a 64 caratteri, senza a capo.';

  @override
  String get theNameCouldNotBeSaved => 'Impossibile salvare il nome. Riprova.';

  @override
  String get thisInviteHasExpiredMakeA =>
      'Questo invito è scaduto. Creane uno nuovo sull\'altro dispositivo.';

  @override
  String get thisInviteWasMadeOnThis =>
      'Questo invito è stato creato su questo dispositivo. Usalo sull\'altro.';

  @override
  String get thatDeviceIsAlreadyPaired => 'Quel dispositivo è già associato.';

  @override
  String get thatIsNotAValidInvite =>
      'Non è un invito valido. Copialo di nuovo.';

  @override
  String get deviceSyncIsNotRunning =>
      'La sincronizzazione dei dispositivi non è attiva.';

  @override
  String get theOtherDeviceIsNoLonger =>
      'L\'altro dispositivo non sta più associando. Riprova.';

  @override
  String get theOtherAppCouldNotBe => 'Impossibile aprire l\'altra app.';

  @override
  String get theOtherApp => 'l\'altra app';

  @override
  String app(Object displayName) {
    return 'app $displayName';
  }

  @override
  String get unableToChangeTheLogLevel =>
      'Impossibile modificare il livello dei log';

  @override
  String logsExportedTo(Object filePath) {
    return 'Log esportati in $filePath';
  }

  @override
  String get unableToShareTheLogs => 'Impossibile condividere i log';

  @override
  String get noLogsToExportYet => 'Ancora nessun log da esportare';

  @override
  String get unableToExportTheLogs => 'Impossibile esportare i log';

  @override
  String couldNotBeAttached(Object attached, Object failedPaths) {
    return '$attached; $failedPaths non allegati';
  }

  @override
  String get theFilesCouldNotBeAttached => 'Impossibile allegare i file.';

  @override
  String get unableToShareTheExport =>
      'Impossibile condividere l\'esportazione';

  @override
  String exportedTo(Object filePath) {
    return 'Esportato in $filePath';
  }

  @override
  String get exportSaved => 'Esportazione salvata';

  @override
  String get unableToSaveTheExport => 'Impossibile salvare l\'esportazione';

  @override
  String get thisMeetingNoLongerExists => 'Questa riunione non esiste più';

  @override
  String get exportIsUnavailableUntilTheApp =>
      'L\'esportazione non è disponibile finché l\'app non è avviata';

  @override
  String get unableToExportTheMeeting => 'Impossibile esportare la riunione';

  @override
  String get unableToLoadMeetings => 'Impossibile caricare le riunioni';

  @override
  String get unableToLoadThisMeeting => 'Impossibile caricare questa riunione';

  @override
  String get enterATitle => 'Inserisci un titolo.';

  @override
  String titlesCanBeAtMostCharacters(
      Object maxMeetingTitleCharacters, Object runes) {
    return 'I titoli possono avere al massimo $maxMeetingTitleCharacters caratteri (ne hai $runes).';
  }

  @override
  String get titlesCanTContainLineBreaks =>
      'I titoli non possono contenere a capo o caratteri di controllo.';

  @override
  String get notPairedYetMakeTheRelay =>
      'Non ancora associato. Rendi il relay rilevabile, poi associalo da Dispositivi vicini in Dispositivi.';

  @override
  String get connectingToTheRelay => 'Connessione al relay…';

  @override
  String get connectedToTheRelayAskingIt =>
      'Connesso al relay. Gli chiedo di accettare le connessioni…';

  @override
  String get connectedYourOtherDevicesCanReach =>
      'Connesso. Gli altri tuoi dispositivi possono raggiungere questo tramite il relay.';

  @override
  String get savedThisAppliesNowWithNo =>
      'Salvato. Vale da subito, senza riavviare.';

  @override
  String get thatRelayAddressIsNotValid =>
      'Quell\'indirizzo del relay non è valido. Ha questa forma: /ip4/203.0.113.5/tcp/4001/p2p/12D3Koo…';

  @override
  String get aDeviceAddressIsNotValid =>
      'Un indirizzo di dispositivo non è valido. Ognuno termina con /p2p/ e l\'ID del dispositivo.';

  @override
  String get unableToSaveTheAddresses => 'Impossibile salvare gli indirizzi.';

  @override
  String get thePhotoCouldNotBeSaved => 'Impossibile salvare la foto.';

  @override
  String get enterAName => 'Inserisci un nome.';

  @override
  String namesCanBeAtMostCharacters(Object maxMeetingTitleCharacters) {
    return 'I nomi possono avere al massimo $maxMeetingTitleCharacters caratteri.';
  }

  @override
  String get namesCanTContainLineBreaks =>
      'I nomi non possono contenere a capo o caratteri di controllo.';

  @override
  String get unableToLoadProjects => 'Impossibile caricare i progetti';

  @override
  String get unableToLoadThisProject => 'Impossibile caricare questo progetto';

  @override
  String get theFilesCouldNotBeAdded => 'Impossibile aggiungere i file.';

  @override
  String couldNotBeAdded(Object failedPaths) {
    return '$failedPaths non aggiunti';
  }

  @override
  String get unableToStartRecording => 'Impossibile avviare la registrazione';

  @override
  String get unableToPauseRecording =>
      'Impossibile mettere in pausa la registrazione';

  @override
  String get unableToResumeRecording =>
      'Impossibile riprendere la registrazione';

  @override
  String get unableToStopRecording => 'Impossibile fermare la registrazione';

  @override
  String get recordingSaved => 'Registrazione salvata';

  @override
  String get microphoneAccessIsNeededToRecord =>
      'Per registrare serve l\'accesso al microfono';

  @override
  String get keepTheAppOpenAndroidDid =>
      'Tieni aperta l\'app: Android non ha consentito la registrazione in background';

  @override
  String get unableToOpenTheMicrophone => 'Impossibile aprire il microfono';

  @override
  String saved(Object label) {
    return '$label salvata.';
  }

  @override
  String removed(Object label) {
    return '$label rimossa.';
  }

  @override
  String isNowTheDefault(Object id) {
    return '$id è ora la predefinita.';
  }

  @override
  String answered(Object id, Object reply) {
    return '$id ha risposto: $reply';
  }

  @override
  String get promptSaved => 'Prompt salvato.';

  @override
  String get theBuiltInPromptIsBack => 'È tornato il prompt predefinito.';

  @override
  String get finishedMeetingsWillBeSummarizedAutomatically =>
      'Le riunioni concluse verranno riassunte automaticamente.';

  @override
  String get automaticSummariesAreOff =>
      'I riassunti automatici sono disattivati.';

  @override
  String get thisComputerNoLongerWritesSummaries =>
      'Questo computer non scrive più riassunti né risponde alle chat per gli altri tuoi dispositivi.';

  @override
  String nowWritesTheSummariesAndAnswers(Object id) {
    return '$id ora scrive i riassunti e risponde alle chat richiesti dagli altri tuoi dispositivi.';
  }

  @override
  String get requestCancelled => 'Richiesta annullata.';

  @override
  String get thisMeetingUsesTheSettingsPrompt =>
      'Questa riunione usa di nuovo il prompt delle Impostazioni.';

  @override
  String get promptSavedForThisMeeting => 'Prompt salvato per questa riunione.';

  @override
  String get summarySaved => 'Riassunto salvato.';

  @override
  String get theConnection => 'La connessione';

  @override
  String get summaryWritten => 'Riassunto scritto.';

  @override
  String theTranscriptWasReadInParts(Object parts) {
    return ' La trascrizione è stata letta in $parts parti.';
  }

  @override
  String leftOut(Object sent, Object imagesSkipped) {
    return ' $sent, $imagesSkipped escluse.';
  }

  @override
  String get theModelStoppedBeforeTheEnd =>
      ' Il modello si è fermato prima della fine.';

  @override
  String get waitingForYourComputerItWrites =>
      'In attesa del tuo computer. Scriverà il riassunto quando sarà acceso, connesso a questo dispositivo e impostato per rispondere in Impostazioni → Connessioni IA.';

  @override
  String get yourComputerIsWritingTheSummary =>
      'Il tuo computer sta scrivendo il riassunto…';

  @override
  String yourComputerCouldNotWriteThe(Object message) {
    return 'Il tuo computer non è riuscito a scrivere il riassunto: $message';
  }

  @override
  String get unableToRefreshTelemetry => 'Impossibile aggiornare la telemetria';

  @override
  String get automatic => 'Automatica';

  @override
  String get transcriptionLanguage => 'Lingua di trascrizione';

  @override
  String get transcriptionLanguages => 'Lingue di trascrizione';

  @override
  String get theLanguageCouldNotBeChanged => 'Impossibile cambiare la lingua.';

  @override
  String get thisDevice => 'Questo dispositivo';

  @override
  String get bestAvailable => 'Il migliore disponibile';

  @override
  String get aRemovedDevice => 'Un dispositivo rimosso';

  @override
  String get theTranscriptionDeviceCouldNotBe =>
      'Impossibile cambiare il dispositivo di trascrizione. Verifica che sia ancora associato.';

  @override
  String get downloadFinishedYouCanNowChoose =>
      'Download completato. Ora puoi scegliere questo modello.';

  @override
  String get downloadCancelled => 'Download annullato.';

  @override
  String get theDownloadStoppedNoConnectionTo =>
      'Il download si è interrotto: nessuna connessione al server. Il modello attuale funziona ancora.';

  @override
  String get theDownloadedFileFailedItsCheck =>
      'Il file scaricato non ha superato la verifica ed è stato eliminato. Il modello attuale funziona ancora.';

  @override
  String get theModelCouldNotBeSaved =>
      'Impossibile salvare il modello. Controlla lo spazio libero. Il modello attuale funziona ancora.';

  @override
  String get theDownloadServerRefusedTheFile =>
      'Il server di download ha rifiutato il file. Il modello attuale funziona ancora.';

  @override
  String get savedTheNextRecordingUsesThis =>
      'Salvato. La prossima registrazione userà questo modello.';

  @override
  String get savedTheNextRecordingUsesThis2 =>
      'Salvato. La prossima registrazione lo userà.';

  @override
  String get savedTheNextRecordingUsesThe =>
      'Salvato. La prossima registrazione userà la scelta automatica.';

  @override
  String get savedMeetingsRecordedFromNowOn =>
      'Salvato. Le riunioni registrate d\'ora in poi lo useranno.';

  @override
  String get savedTheNextRecordingUsesThese =>
      'Salvato. La prossima registrazione userà questi nomi e termini.';

  @override
  String get eachTermCanHaveAtMost =>
      'Ogni termine può avere al massimo 60 caratteri, e tutti insieme al massimo 500.';

  @override
  String get chooseAWindowBetween1And => 'Scegli un periodo tra 1 e 60 giorni.';

  @override
  String get theDownloadCouldNotStartPlease =>
      'Impossibile avviare il download. Riprova.';

  @override
  String get savedTheNextRecordingIsTranscribed =>
      'Salvato. La prossima registrazione verrà trascritta sulla NPU.';

  @override
  String get savedTheNextRecordingIsTranscribed2 =>
      'Salvato. La prossima registrazione verrà trascritta sul processore.';

  @override
  String get theNpuFilesAreNotAll =>
      'I file per la NPU non sono ancora stati scaricati tutti.';

  @override
  String get theNpuFilesWereRemoved => 'I file per la NPU sono stati rimossi.';

  @override
  String get theNpuFilesCouldNotBe =>
      'Impossibile rimuovere i file per la NPU. Riprova.';

  @override
  String get downloadedRecordingsNowTranscribeOnThe =>
      'Scaricati. Ora le registrazioni vengono trascritte sulla NPU.';

  @override
  String get theDownloadStoppedNoConnectionTo2 =>
      'Il download si è interrotto: nessuna connessione al server.';

  @override
  String get aDownloadedFileFailedItsCheck =>
      'Un file scaricato non ha superato la verifica ed è stato eliminato.';

  @override
  String get theFilesCouldNotBeSaved =>
      'Impossibile salvare i file. Controlla lo spazio libero (servono circa 800 MB).';

  @override
  String get theDownloadServerRefusedTheFiles =>
      'Il server di download ha rifiutato i file.';

  @override
  String get theNextRecordingTriesTheGraphics =>
      'La prossima registrazione proverà di nuovo il chip grafico.';

  @override
  String get thatModelWonTBeProposed => 'Quel modello non verrà più proposto.';

  @override
  String get modelRemoved => 'Modello rimosso.';

  @override
  String get anotherModelIsDownloadingWaitFor =>
      'È in download un altro modello. Attendi che finisca.';

  @override
  String get thereIsnTEnoughFreeSpace =>
      'Non c\'è abbastanza spazio libero per questo modello.';

  @override
  String get thatDidnTWorkPleaseTry => 'Non ha funzionato. Riprova.';

  @override
  String get unableToLoadVoiceGallery =>
      'Impossibile caricare la Galleria voci';

  @override
  String get unableToPlayThisVoice => 'Impossibile riprodurre questa voce';

  @override
  String get unableToDeleteThisVoice => 'Impossibile eliminare questa voce';

  @override
  String get voiceDeleted => 'Voce eliminata';

  @override
  String get unableToDeleteThisPerson => 'Impossibile eliminare questa persona';

  @override
  String deleted(Object displayName) {
    return '$displayName eliminato';
  }

  @override
  String get enterANameBeforeAssigning =>
      'Inserisci un nome prima di assegnare';

  @override
  String assignedTo(Object identityName) {
    return 'Assegnato a $identityName';
  }

  @override
  String assignedToThereWasNotEnough(Object identityName) {
    return 'Assegnato a $identityName. Non c\'era abbastanza parlato chiaro per riconoscere questa voce nelle registrazioni future.';
  }

  @override
  String get unableToAssignTheSpeakerCluster =>
      'Impossibile assegnare il gruppo di interventi';

  @override
  String get chooseAtLeastOneVoiceTo => 'Scegli almeno una voce da nominare';

  @override
  String thereWasNotEnoughClearSpeech(Object assigned) {
    return '$assigned. Non c\'era abbastanza parlato chiaro per riconoscere questa voce nelle registrazioni future.';
  }

  @override
  String get theSettingCouldNotBeSaved =>
      'Impossibile salvare l\'impostazione.';

  @override
  String get theAppIsStillStartingTry =>
      'L\'app si sta ancora avviando. Riprova.';

  @override
  String get somethingWentWrongTryAgain => 'Qualcosa è andato storto. Riprova.';

  @override
  String isUsingTheMicrophone(Object app) {
    return '$app sta usando il microfono';
  }

  @override
  String get recordThisCall => 'Registrare questa chiamata?';

  @override
  String theAiServerRejectedTheRequest(Object detail) {
    return 'Il server IA ha rifiutato la richiesta.\nIl server ha detto: $detail';
  }

  @override
  String claudeCodeOrCodexCouldNot(Object detail) {
    return 'Claude Code o Codex non è riuscito a rispondere.\nHa detto: $detail';
  }

  @override
  String get typeAQuestion => 'Scrivi una domanda.';

  @override
  String get theQuestionIsTooLong => 'La domanda è troppo lunga.';

  @override
  String get addAnAiConnectionInSettings =>
      'Aggiungi una connessione IA nelle Impostazioni, o scegline una predefinita.';

  @override
  String get thatAiConnectionNoLongerExists =>
      'Quella connessione IA non esiste più.';

  @override
  String get thereIsNothingToTalkAbout =>
      'Non c\'è ancora nulla di cui parlare: nessuna trascrizione, riassunto, nota o file.';

  @override
  String get theAiConnectionSContextIs =>
      'Il contesto della connessione IA è troppo piccolo. Scegline una con un contesto più ampio.';

  @override
  String get thatMeetingNoLongerExists => 'Quella riunione non esiste più.';

  @override
  String get thatProjectNoLongerExists => 'Quel progetto non esiste più.';

  @override
  String get theAiServerCouldNotBe =>
      'Impossibile raggiungere il server IA. Controlla la connessione.';

  @override
  String get theAiServerRefusedTheApi =>
      'Il server IA ha rifiutato la chiave API.';

  @override
  String get theAiServerIsBusyTry =>
      'Il server IA è occupato. Riprova più tardi.';

  @override
  String get theModelDeclinedToAnswer =>
      'Il modello si è rifiutato di rispondere.';

  @override
  String get claudeCodeOrCodexIsNot =>
      'Claude Code o Codex non è installato su questo computer, oppure l\'app non lo trova.';

  @override
  String get theNotesCouldNotBeRead => 'Impossibile leggere le note.';

  @override
  String get noPairedComputerAnsweredOnA =>
      'Nessun computer associato ha risposto. Su un computer, apri Impostazioni → IA, scegli una connessione in \"Riassunti per gli altri miei dispositivi\" e tienilo acceso e connesso.';

  @override
  String get theComputerTookTooLongTo =>
      'Il computer ha impiegato troppo tempo a rispondere.';

  @override
  String get theComputerDisconnectedBeforeAnsweringTry =>
      'Il computer si è disconnesso prima di rispondere. Riprova.';

  @override
  String get theComputerAnswersAtMost30 =>
      'Il computer risponde al massimo a 30 domande all\'ora. Riprova più tardi.';

  @override
  String get theComputerIsAnsweringOtherQuestions =>
      'Il computer sta rispondendo ad altre domande. Riprova tra poco.';

  @override
  String get pairedDevicesAreOffOnThis =>
      'I dispositivi associati sono disattivati su questo telefono.';

  @override
  String get theAiServerCouldNotAnswer =>
      'Il server IA non è riuscito a rispondere. Riprova.';

  @override
  String get exportMeeting => 'Esporta riunione';

  @override
  String get openaiCompatibleServer => 'Server compatibile con OpenAI';

  @override
  String get localServerOllama => 'Server locale (Ollama)';

  @override
  String get claudeCodeClaudePlan => 'Claude Code (abbonamento Claude)';

  @override
  String get codexChatgptPlan => 'Codex (abbonamento ChatGPT)';

  @override
  String get checkTheAddressAndModelPlain =>
      'Controlla indirizzo e modello. Il semplice http:// funziona solo su questo dispositivo o sulla rete locale.';

  @override
  String get thatConnectionNoLongerExists =>
      'Quella connessione non esiste più.';

  @override
  String get addAnAiConnectionInSettings2 =>
      'Aggiungi prima una connessione IA nelle Impostazioni.';

  @override
  String get thisMeetingHasNoTranscriptOr =>
      'Questa riunione non ha ancora trascrizione né note.';

  @override
  String get thisMeetingIsTooLongFor =>
      'Questa riunione è troppo lunga per questa connessione. Scegline una con un contesto più ampio.';

  @override
  String get theAiServerCouldNotBe2 => 'Impossibile raggiungere il server IA.';

  @override
  String get theAiServerRefusedAccessUse =>
      'Il server IA ha rifiutato l\'accesso. Usa un server che non richiede una chiave API.';

  @override
  String get theAiServerIsBusyOr =>
      'Il server IA è occupato o ha superato il limite. Riprova più tardi.';

  @override
  String get theAiServerRejectedTheRequest2 =>
      'Il server IA ha rifiutato la richiesta. Controlla il nome del modello.';

  @override
  String get theAiServerFailedTryAgain =>
      'Il server IA ha avuto un errore. Riprova.';

  @override
  String get theAiServerSAnswerCould =>
      'Impossibile leggere la risposta del server IA.';

  @override
  String get claudeCodeOrCodexCouldNot2 =>
      'Claude Code o Codex non è riuscito a rispondere. Verifica di aver effettuato l\'accesso.';

  @override
  String get theSettingsCouldNotBeSaved =>
      'Impossibile salvare le impostazioni.';

  @override
  String get theMeetingCouldNotBeRead =>
      'Impossibile leggere o salvare la riunione.';

  @override
  String get error => 'Errore';

  @override
  String get warning => 'Avviso';

  @override
  String get infoDefault => 'Info (predefinito)';

  @override
  String get debug => 'Debug';

  @override
  String get trace => 'Trace';

  @override
  String get thisDevice2 => 'questo dispositivo';

  @override
  String get anotherDevice => 'un altro dispositivo';

  @override
  String notConnected(Object label) {
    return '$label (non connesso)';
  }

  @override
  String itIsKeptEncryptedUntil(Object until) {
    return ' È conservato cifrato fino al $until.';
  }

  @override
  String get recordedOnThisDeviceOnly =>
      'Registrata solo su questo dispositivo.';

  @override
  String recordedOnThisDeviceOnlyA(Object until) {
    return 'Registrata solo su questo dispositivo. Una sua registrazione fatta su un altro dispositivo può ancora essere combinata fino al $until.';
  }

  @override
  String combinesTheRecordingsNothingIsLeft(Object combiner) {
    return '$combiner combina le registrazioni; da qui non resta nulla da inviare.';
  }

  @override
  String waitingForAudioFrom(Object names, Object kept) {
    return 'In attesa dell\'audio da $names.$kept';
  }

  @override
  String combinesTheRecordingsWaitingForAudio(Object combiner, Object names) {
    return '$combiner combina le registrazioni; in attesa dell\'audio da $names.';
  }

  @override
  String get audioFromEveryDeviceIsHere =>
      'È arrivato l\'audio di tutti i dispositivi; la trascrizione viene combinata quando questo dispositivo è libero.';

  @override
  String thisDeviceSAudioStillHas(Object combiner, Object kept) {
    return 'L\'audio di questo dispositivo deve ancora raggiungere $combiner.$kept';
  }

  @override
  String get transcriptCombinedFromEveryDevice =>
      'Trascrizione combinata da tutti i dispositivi.';

  @override
  String notCombinedAudioFromDidNot(Object plainNames) {
    return 'Non combinata: l\'audio da $plainNames non è arrivato entro tre settimane.';
  }

  @override
  String meeting(Object createdAt) {
    return 'Riunione $createdAt';
  }

  @override
  String get writeSomethingFirst => 'Scrivi prima qualcosa.';

  @override
  String get theTextIsTooLong => 'Il testo è troppo lungo.';

  @override
  String get thatPartOfTheTranscriptChanged =>
      'Quella parte della trascrizione è cambiata nel frattempo. Riapri la riunione.';

  @override
  String get theMeetingCouldNotBeSaved => 'Impossibile salvare la riunione.';

  @override
  String get chooseAFullFolderPathInside =>
      'Scegli il percorso completo di una cartella, dentro la tua cartella home ma non la home stessa.';

  @override
  String get theFolderCouldNotBeWritten =>
      'Impossibile scrivere nella cartella. Verifica che esista e che tu possa scriverci.';

  @override
  String get theNotesCouldNotBeRead2 =>
      'Impossibile leggere o salvare le note.';

  @override
  String get theInviteAnswerIsIncomplete =>
      'La risposta all\'invito è incompleta.';

  @override
  String get theRenameAnswerIsIncomplete =>
      'La risposta alla rinomina è incompleta.';

  @override
  String get thisDeviceHasNoCamera =>
      'Questo dispositivo non ha una fotocamera.';

  @override
  String get theCameraIsNotOpen => 'La fotocamera non è aperta.';

  @override
  String get title => 'Titolo';

  @override
  String get summary => 'Riassunto';

  @override
  String get notes => 'Note';

  @override
  String get transcript => 'Trascrizione';

  @override
  String get projectName => 'Nome del progetto';

  @override
  String file(Object fileName) {
    return 'File $fileName';
  }

  @override
  String get typeWhatToLookFor => 'Scrivi cosa cercare.';

  @override
  String get typeAtLeastTwoLetters => 'Scrivi almeno due lettere.';

  @override
  String get theSearchIsTooLong => 'La ricerca è troppo lunga.';

  @override
  String get exportLogs => 'Esporta i log';

  @override
  String about(Object appName) {
    return 'Informazioni su $appName';
  }

  @override
  String get versionLicenseAndTheSoftwareIt =>
      'Versione, licenza e software utilizzato.';

  @override
  String remove(Object label) {
    return 'Rimuovere $label?';
  }

  @override
  String get itsApiKeyIsDeletedFrom =>
      'La sua chiave API viene eliminata da questo dispositivo. I riassunti già scritti restano.';

  @override
  String get cancel => 'Annulla';

  @override
  String get remove2 => 'Rimuovi';

  @override
  String get aiConnections => 'Connessioni IA';

  @override
  String get addConnection => 'Aggiungi connessione';

  @override
  String get summariesSendTheMeetingSTranscript =>
      'I riassunti inviano trascrizione, note e foto della riunione al server della connessione. Un server sulla tua rete locale li tiene lì.';

  @override
  String get noConnectionsYet => 'Ancora nessuna connessione';

  @override
  String get addAServerOnYourNetwork =>
      'Aggiungi un server sulla tua rete (Ollama, LM Studio, llama.cpp), oppure Claude Code o Codex su questo computer.';

  @override
  String get summarizeMeetingsAutomatically =>
      'Riassumi le riunioni automaticamente';

  @override
  String get withTheDefaultConnectionWhenA =>
      'Con la connessione predefinita, quando la trascrizione di una riunione è definitiva.';

  @override
  String get summariesForMyOtherDevices =>
      'Riassunti per gli altri miei dispositivi';

  @override
  String get aPairedPhoneCanAskThis =>
      'Un telefono associato può chiedere a questo computer di scrivere un riassunto o rispondere a una chat con questa connessione, dalla copia delle tue note su questo computer. Conta sul tuo abbonamento; al massimo 10 riassunti e 30 risposte in chat all\'ora.';

  @override
  String get off => 'Disattivato';

  @override
  String get summaryPrompt => 'Prompt del riassunto';

  @override
  String placeholdersTheTranscriptIsAlwaysSent(Object placeholders) {
    return 'Segnaposto: $placeholders. La trascrizione viene sempre inviata dopo il prompt.';
  }

  @override
  String get savePrompt => 'Salva prompt';

  @override
  String get resetToBuiltIn => 'Ripristina predefinito';

  @override
  String get defaultLabel => 'Predefinita';

  @override
  String get installed => 'Installato';

  @override
  String get notFoundOnThisComputer => 'Non trovato su questo computer';

  @override
  String get unencryptedHttpLocalNetworkOnly =>
      'http non cifrato, solo rete locale';

  @override
  String get runsWithYourPlanSSign =>
      'Usa l\'accesso del tuo abbonamento su questo computer';

  @override
  String actionsFor(Object label) {
    return 'Azioni per $label';
  }

  @override
  String get makeDefault => 'Rendi predefinita';

  @override
  String get test => 'Prova';

  @override
  String get edit => 'Modifica';

  @override
  String get enterANameAnAddressAnd =>
      'Inserisci un nome, un indirizzo e un modello.';

  @override
  String get theContextIsANumberOf =>
      'Il contesto è un numero di token, per esempio 8192.';

  @override
  String edit2(Object label) {
    return 'Modifica $label';
  }

  @override
  String get name => 'Nome';

  @override
  String get protocol => 'Protocollo';

  @override
  String get runsTheToolInstalledAndSigned =>
      'Usa lo strumento installato e con accesso effettuato su questo computer, così i riassunti li scrive il tuo abbonamento Claude o ChatGPT invece del credito API. La riunione va ad Anthropic o OpenAI con quell\'account.';

  @override
  String get address => 'Indirizzo';

  @override
  String get httpsAlwaysHttpOnlyOnThis =>
      'https:// sempre; http:// solo su questo dispositivo o sulla rete locale.';

  @override
  String get model => 'Modello';

  @override
  String get leaveEmptyForTheToolS =>
      'Lascia vuoto per il modello predefinito dello strumento.';

  @override
  String get contextTokens => 'Contesto (token)';

  @override
  String get saveConnection => 'Salva connessione';

  @override
  String get koalinoIsLocked => 'Koalino è bloccato';

  @override
  String get unlockWithYourFingerprintFaceOr =>
      'Sblocca con impronta, volto o blocco schermo.';

  @override
  String get unlock => 'Sblocca';

  @override
  String copied(Object what) {
    return '$what copiato';
  }

  @override
  String get aiAssistantAccess => 'Accesso per l\'assistente IA';

  @override
  String get letYourOwnAiAssistantSuch =>
      'Consenti al tuo assistente IA, come Claude Code, Codex o Claude Desktop, di leggere riunioni e progetti: titoli, riassunti, note, trascrizioni e file dei progetti. Può solo leggere; non può modificare né eliminare nulla.';

  @override
  String get whatTheAssistantReadsMayBe =>
      'Ciò che l\'assistente legge può essere inviato al suo modello online, con il tuo account presso quel servizio. Attivalo solo per un assistente a cui affideresti le tue riunioni.';

  @override
  String get theAssistantServerIsNotInstalled =>
      'Il server per l\'assistente non è installato con questa copia dell\'app.';

  @override
  String get letMyAiAssistantReadMy =>
      'Consenti al mio assistente IA di leggere le mie note';

  @override
  String get connectYourAssistant => 'Collega il tuo assistente';

  @override
  String get yourNotesAreUnlockedWithA =>
      'Le tue note si sbloccano con una passphrase, che non viene mai scritta nelle impostazioni di un assistente. Il server le legge solo se NOTE_TAKER_ENCRYPTION_PASSPHRASE è impostata dove l\'assistente lo avvia.';

  @override
  String get claudeCodeRunInATerminal => 'Claude Code: esegui in un terminale';

  @override
  String get command => 'Comando';

  @override
  String get claudeDesktopAndOtherMcpClients =>
      'Claude Desktop e altri client MCP: aggiungi alle loro impostazioni';

  @override
  String get settings => 'Impostazioni';

  @override
  String copy(Object title) {
    return 'Copia: $title';
  }

  @override
  String isUsingTheMicrophoneRecordThis(Object app) {
    return '$app sta usando il microfono. Registrare questa chiamata?';
  }

  @override
  String get notNow => 'Non ora';

  @override
  String get record => 'Registra';

  @override
  String get offerToRecordCalls => 'Proponi di registrare le chiamate';

  @override
  String get whenTeamsZoomMeetInA =>
      'Quando Teams, Zoom, Meet in un browser o un\'altra app di chiamate inizia a usare il microfono. Non si registra nulla finché non tocchi Registra.';

  @override
  String get stopWhenTheCallEnds => 'Ferma quando la chiamata termina';

  @override
  String get onlyForRecordingsStartedFromThe =>
      'Solo per le registrazioni avviate dalla proposta, quando l\'app di chiamata ha lasciato il microfono da 20 secondi.';

  @override
  String get readSpeakerNamesFromTheCall =>
      'Leggi i nomi di chi parla dalla finestra della chiamata';

  @override
  String get whileRecordingKoalinoReadsWhoTeams =>
      'Durante la registrazione, Koalino legge chi Teams, Zoom o Meet mostra mentre parla, per suggerire nomi per le voci sconosciute. Nulla lascia questo computer e nessuno viene nominato finché non tocchi Assegna.';

  @override
  String get deleteThisChat => 'Eliminare questa chat?';

  @override
  String get itsQuestionsAndAnswersAreDeleted =>
      'Domande e risposte vengono eliminate da questo dispositivo.';

  @override
  String get delete => 'Elimina';

  @override
  String get thatFileIsNoLongerIn => 'Quel file non è più nel progetto.';

  @override
  String get whoAnswers => 'Chi risponde';

  @override
  String get myPairedComputer => 'Il mio computer associato';

  @override
  String get thisPhoneSAiConnection => 'La connessione IA di questo telefono';

  @override
  String get deleteThisChat2 => 'Elimina questa chat';

  @override
  String get askAboutYourNotesAnswersCome =>
      'Fai domande sulle tue note. Risponde il tuo computer associato, che legge la propria copia delle note con la connessione scelta lì per gli altri tuoi dispositivi.';

  @override
  String get askAboutYourNotesTheAnswer =>
      'Fai domande sulle tue note. Risponde la tua connessione IA predefinita, a cui vengono inviate le note che ci stanno.';

  @override
  String get yourQuestion => 'La tua domanda';

  @override
  String get ask => 'Chiedi';

  @override
  String get open => 'Apri';

  @override
  String given(Object included) {
    return 'Fornito: $included';
  }

  @override
  String leftOutForLength(Object omitted) {
    return 'Escluso per lunghezza: $omitted';
  }

  @override
  String answeredBy(Object connection) {
    return 'Risposta di $connection';
  }

  @override
  String get unableToOpenTheKeychain => 'Impossibile aprire il portachiavi';

  @override
  String get koalinoKeepsTheKeyThatEncrypts =>
      'Koalino tiene nel portachiavi di sistema la chiave che cifra i tuoi profili vocali e l\'identità del dispositivo. Sblocca il portachiavi, o consenti a Koalino di usarlo, poi riprova. Se questo dispositivo non ha un servizio portachiavi, installane o avviane uno e riavvia Koalino.';

  @override
  String get unableToOpenYourNotes => 'Impossibile aprire le tue note';

  @override
  String get koalinoCouldNotOpenItsDatabase =>
      'Koalino non è riuscito ad aprire il suo database. Verifica che ci sia spazio libero e riprova.';

  @override
  String get unlockingYourNotes => 'Sblocco delle note…';

  @override
  String get quit => 'Esci';

  @override
  String get retry => 'Riprova';

  @override
  String delete2(Object title) {
    return 'Eliminare $title?';
  }

  @override
  String get itsTranscriptAttachmentsAndUnknownVoices =>
      'Trascrizione, allegati e voci sconosciute vengono eliminati definitivamente, qui e sui dispositivi associati. Le persone salvate restano.';

  @override
  String get unableToDeleteThisMeetingIf =>
      'Impossibile eliminare questa riunione. Se è in registrazione, ferma prima la registrazione.';

  @override
  String get devices => 'Dispositivi';

  @override
  String get deviceSyncIsOff =>
      'La sincronizzazione dei dispositivi è disattivata';

  @override
  String get meetingsStayOnThisDeviceUntil =>
      'Le riunioni restano su questo dispositivo finché non attivi la sincronizzazione.';

  @override
  String thisDevice3(Object capabilities) {
    return 'Questo dispositivo\n$capabilities';
  }

  @override
  String get renameThisDevice => 'Rinomina questo dispositivo';

  @override
  String get discoverableYourOtherDevicesCanAsk =>
      'Rilevabile: per qualche minuto gli altri tuoi dispositivi possono chiedere di associarsi.';

  @override
  String get makeDiscoverable => 'Rendi rilevabile';

  @override
  String get pairingRequests => 'Richieste di associazione';

  @override
  String get acceptOnlyIfTheOtherDevice =>
      'Accetta solo se l\'altro dispositivo mostra lo stesso codice.';

  @override
  String get reject => 'Rifiuta';

  @override
  String get accept => 'Accetta';

  @override
  String get pairingInProgress => 'Associazione in corso';

  @override
  String get waitingForTheOtherDevice => 'In attesa dell\'altro dispositivo…';

  @override
  String get checkThatTheOtherDeviceShows =>
      'Verifica che l\'altro dispositivo mostri lo stesso codice.';

  @override
  String get cancelPairing => 'Annulla associazione';

  @override
  String get codesMatch => 'I codici coincidono';

  @override
  String get pairedDevices => 'Dispositivi associati';

  @override
  String get noPairedDevicesYet => 'Ancora nessun dispositivo associato.';

  @override
  String paired(Object pairedAt, Object hasRelay) {
    return 'Associato il $pairedAt\n$hasRelay';
  }

  @override
  String remove3(Object deviceName) {
    return 'Rimuovi $deviceName';
  }

  @override
  String get nearbyDevices => 'Dispositivi vicini';

  @override
  String get pair => 'Associa';

  @override
  String get anotherNetwork => 'Un\'altra rete';

  @override
  String get showAnInviteHereAndUse =>
      'Mostra un invito qui e usalo sull\'altro dispositivo. Confronterai comunque un codice prima dell\'associazione.';

  @override
  String get showInvite => 'Mostra invito';

  @override
  String get useInvite => 'Usa invito';

  @override
  String stopsSyncingWithThisDevicePair(Object deviceName) {
    return '$deviceName smette di sincronizzarsi con questo dispositivo. Associalo di nuovo per riprendere.';
  }

  @override
  String pairingCode(Object split) {
    return 'Codice di associazione $split';
  }

  @override
  String get noOtherDevicesFoundOnThis =>
      'Nessun altro dispositivo trovato su questa rete. Vicini trova solo i dispositivi sulla stessa Wi-Fi. Per un dispositivo su un\'altra rete, usa un invito.';

  @override
  String get noOtherDevicesFoundOnThis2 =>
      'Nessun altro dispositivo trovato su questa rete. Vicini trova solo i dispositivi sulla stessa Wi-Fi. Per un dispositivo su un\'altra rete, usa un invito o imposta un relay nelle Impostazioni.';

  @override
  String get notConnectedItReconnectsWhenIt =>
      'Non connesso. Si riconnette quando è online e raggiunge il tuo relay.';

  @override
  String get notConnectedPutBothDevicesOn =>
      'Non connesso. Metti entrambi i dispositivi sulla stessa Wi-Fi, o imposta un relay nelle Impostazioni.';

  @override
  String get whisperOnDevice => 'Whisper (sul dispositivo)';

  @override
  String get openaiCloud => 'OpenAI (cloud)';

  @override
  String gbGpu(Object round) {
    return 'GPU $round GB';
  }

  @override
  String mbGpu(Object gpuMegabytes) {
    return 'GPU $gpuMegabytes MB';
  }

  @override
  String get noTranscriptionModels => 'Nessun modello di trascrizione';

  @override
  String get useYourOwnAiApp => 'Usa la tua app di IA';

  @override
  String get theMeetingItsNotesAndIts =>
      'La riunione, le sue note e i suoi file vanno all\'app che scegli, con il tuo account. Per un riassunto, copia lì la risposta e torna qui, oppure condividila con Koalino: Koalino la conserva.';

  @override
  String summarizeIn(Object displayName) {
    return 'Riassumi in $displayName';
  }

  @override
  String chatIn(Object displayName) {
    return 'Chatta in $displayName';
  }

  @override
  String get summarizeInAnotherApp => 'Riassumi in un\'altra app…';

  @override
  String get chatInAnotherApp => 'Chatta in un\'altra app…';

  @override
  String summaryFromSavedTo(Object appName, Object title) {
    return 'Riassunto da $appName salvato in \"$title\".';
  }

  @override
  String get undo => 'Annulla';

  @override
  String get theEarlierSummaryIsBack => 'È tornato il riassunto precedente.';

  @override
  String toKeepTheSummaryCopyS(Object appName) {
    return 'Per conservare il riassunto, copia la risposta di $appName e torna su Koalino, oppure condividi la risposta con Koalino.';
  }

  @override
  String get meetings => 'Riunioni';

  @override
  String get projects => 'Progetti';

  @override
  String get voices => 'Voci';

  @override
  String get meeting2 => 'Riunione';

  @override
  String get chatAboutThisMeeting => 'Chatta su questa riunione';

  @override
  String chat(Object meeting) {
    return 'Chat: $meeting';
  }

  @override
  String get thisMeeting => 'questa riunione';

  @override
  String get takePhotos => 'Scatta foto';

  @override
  String get attachFiles => 'Allega file';

  @override
  String get export => 'Esporta';

  @override
  String shareAs(Object label) {
    return 'Condividi come $label';
  }

  @override
  String get nameSpeakers => 'Dai un nome a chi parla';

  @override
  String get selectAMeetingToSeeIts =>
      'Seleziona una riunione per vederne la trascrizione.';

  @override
  String get renameMeeting => 'Rinomina riunione';

  @override
  String get deleteMeeting => 'Elimina riunione';

  @override
  String recordedOn(Object devices) {
    return 'Registrata su $devices';
  }

  @override
  String get split => 'Separa';

  @override
  String get longPressOrRightClickA =>
      'Tieni premuto o fai clic destro su un passaggio per correggerlo.';

  @override
  String get noTranscriptIsAvailable => 'Nessuna trascrizione disponibile.';

  @override
  String get theSpeakerIsAGuess => 'Chi parla è un\'ipotesi';

  @override
  String get peopleTalkedAtOnceHereWords =>
      'Qui più persone parlavano insieme; potrebbero mancare parole';

  @override
  String get overlappingSpeech => 'Parlato sovrapposto';

  @override
  String get someWordsWereUnclear => 'Alcune parole non erano chiare';

  @override
  String get addedAfterTheMeeting => 'Aggiunto dopo la riunione';

  @override
  String photoOpenFullScreen(Object fileName) {
    return 'Foto $fileName, apri a schermo intero';
  }

  @override
  String get save => 'Salva';

  @override
  String get preview => 'Anteprima';

  @override
  String get editNotes => 'Modifica note';

  @override
  String get writeNotes => 'Scrivi note';

  @override
  String get loading => 'Caricamento…';

  @override
  String get noNotesYet => 'Ancora nessuna nota.';

  @override
  String textAt(Object startTimeMs) {
    return 'Testo a $startTimeMs';
  }

  @override
  String get removedThisTranscriptWonTBe =>
      'Rimosso. Questa trascrizione non verrà ritrascritta dopo la riunione.';

  @override
  String get savedThisTranscriptWonTBe =>
      'Salvato. Questa trascrizione non verrà ritrascritta dopo la riunione.';

  @override
  String get editText => 'Modifica testo';

  @override
  String get editTheTranscript => 'Modifica la trascrizione';

  @override
  String get leaveItEmptyToRemoveThis =>
      'Lascia vuoto per rimuovere questa parte.';

  @override
  String get searchMeetings => 'Cerca nelle riunioni';

  @override
  String get chatWithAllYourNotes => 'Chatta con tutte le tue note';

  @override
  String get chatWithAllNotes => 'Chatta con tutte le note';

  @override
  String get refresh => 'Aggiorna';

  @override
  String get noMeetingsYet => 'Ancora nessuna riunione';

  @override
  String get recordingsYouStartAppearHere =>
      'Le registrazioni che avvii compaiono qui.';

  @override
  String get rename => 'Rinomina';

  @override
  String get addToProject => 'Aggiungi a un progetto…';

  @override
  String get summary2 => 'Riassunto…';

  @override
  String get addToProject2 => 'Aggiungi a un progetto';

  @override
  String writingTheSummaryOf(Object title) {
    return 'Scrittura del riassunto di $title';
  }

  @override
  String summaryOf(Object title) {
    return 'Riassunto di $title';
  }

  @override
  String summarizeWith(Object label) {
    return 'Riassumi con $label';
  }

  @override
  String defaultLabel2(Object model) {
    return '$model · predefinita';
  }

  @override
  String get summarizeOnMyComputer => 'Riassumi sul mio computer';

  @override
  String get withClaudeCodeCodexOrThe =>
      'Con Claude Code, Codex o la connessione impostata lì';

  @override
  String get useYourOwnAiApp2 => 'Usa la tua app di IA…';

  @override
  String get claudeChatgptOrAnotherAppWith =>
      'Claude, ChatGPT o un\'altra app, con il tuo account';

  @override
  String get openSummary => 'Apri riassunto';

  @override
  String get editPromptForThisMeeting =>
      'Modifica il prompt per questa riunione…';

  @override
  String get useTheSettingsPrompt => 'Usa il prompt delle Impostazioni';

  @override
  String get promptForThisMeeting => 'Prompt per questa riunione';

  @override
  String placeholders(Object placeholders) {
    return 'Segnaposto: $placeholders';
  }

  @override
  String get addAnAiConnectionInSettings3 =>
      'Aggiungi una connessione IA nelle Impostazioni per scrivere riassunti.';

  @override
  String get summarizeWith2 => 'Riassumi con';

  @override
  String get summarize => 'Riassumi';

  @override
  String get summarizeAgain => 'Riassumi di nuovo';

  @override
  String get editPromptForThisMeeting2 =>
      'Modifica il prompt per questa riunione';

  @override
  String get writingTheSummaryALocalModel =>
      'Scrittura del riassunto… Un modello locale può impiegare qualche minuto.';

  @override
  String get noSummaryYet => 'Ancora nessun riassunto.';

  @override
  String get writeASummary => 'Scrivi un riassunto';

  @override
  String writtenByYouOn(Object createdAt) {
    return 'Scritto da te il $createdAt';
  }

  @override
  String writtenByOn(
      Object model, Object connectionLabel, Object createdAt, Object edited) {
    return 'Scritto da $model ($connectionLabel) il $createdAt$edited';
  }

  @override
  String get editSummary => 'Modifica riassunto';

  @override
  String get tryAgainOnMyComputer => 'Riprova sul mio computer';

  @override
  String get cancelRequest => 'Annulla richiesta';

  @override
  String get notesFolder => 'Cartella delle note';

  @override
  String get keepYourProjectsAndMeetingsAs =>
      'Tieni progetti e riunioni come semplici file in una cartella, da leggere e modificare con qualsiasi editor. Le modifiche a notes.md, summary.md e ai file dei progetti tornano nell\'app entro mezzo minuto, e i file che metti nella cartella di un progetto o in quella degli allegati di una riunione vengono aggiunti. transcript.md viene riscritto dall\'app: correggi la trascrizione nell\'app.';

  @override
  String get theFolderIsADecryptedCopy =>
      'La cartella è una copia decifrata delle tue note. Chiunque possa leggerla può leggerle, e gli strumenti di backup o sincronizzazione la copieranno. Tienila su un disco cifrato, dentro la tua cartella utente.';

  @override
  String get folder => 'Cartella';

  @override
  String get chooseAFolder => 'Scegli una cartella';

  @override
  String get keepTheNotesFolder => 'Mantieni la cartella delle note';

  @override
  String get syncNow => 'Sincronizza ora';

  @override
  String lastSyncAtFailed(Object at, Object code) {
    return 'Ultima sincronizzazione alle $at non riuscita: $code';
  }

  @override
  String lastSyncAtFilesWrittenEdits(
      Object at, Object written, Object imported) {
    return 'Ultima sincronizzazione alle $at: $written file scritti, $imported modifiche e file acquisiti.';
  }

  @override
  String get inviteADevice => 'Invita un dispositivo';

  @override
  String get pairingInviteQrCode => 'Codice QR dell\'invito di associazione';

  @override
  String onTheOtherDeviceOpenDevices(Object hour, Object minute) {
    return 'Sull\'altro dispositivo apri Dispositivi, scegli Usa invito e incolla questo link. Puoi inquadrare il codice con un\'app fotocamera per copiarlo. Funziona fino alle $hour:$minute.';
  }

  @override
  String get close => 'Chiudi';

  @override
  String get inviteLinkCopied => 'Link di invito copiato';

  @override
  String get copyLink => 'Copia link';

  @override
  String get deviceFoundChoosePairUnderNearby =>
      'Dispositivo trovato. Scegli Associa in Dispositivi vicini.';

  @override
  String get useAnInvite => 'Usa un invito';

  @override
  String get inviteLink => 'Link di invito';

  @override
  String get findDevice => 'Trova dispositivo';

  @override
  String get pairWithAnotherDevice => 'Associare un altro dispositivo?';

  @override
  String get youOpenedAPairingInviteKoalino =>
      'Hai aperto un invito di associazione. Koalino troverà il dispositivo che lo mostra, ed entrambi mostreranno un codice da confrontare prima di condividere qualsiasi cosa.';

  @override
  String get pairingStartedCheckThatBothDevices =>
      'Associazione avviata. Verifica che entrambi i dispositivi mostrino lo stesso codice.';

  @override
  String get done => 'Fine';

  @override
  String get photosAreAddedToTheMeeting =>
      'Le foto vengono aggiunte alla riunione man mano che le scatti.';

  @override
  String get takePhoto => 'Scatta foto';

  @override
  String get theCameraIsUnavailable => 'La fotocamera non è disponibile.';

  @override
  String search(Object name) {
    return 'Cerca in $name';
  }

  @override
  String get noteSaved => 'Nota salvata';

  @override
  String get theNoteCouldNotBeSaved => 'Impossibile salvare la nota.';

  @override
  String couldNotBeAdded2(Object failed) {
    return '$failed non aggiunte.';
  }

  @override
  String get renameProject => 'Rinomina progetto';

  @override
  String get theProjectCouldNotBeRenamed =>
      'Impossibile rinominare il progetto.';

  @override
  String delete3(Object project) {
    return 'Eliminare \"$project\"?';
  }

  @override
  String get itsFilesNotesAndEMails =>
      'File, note ed e-mail del progetto vengono eliminati su tutti i dispositivi associati. Le sue riunioni restano, fuori da ogni progetto.';

  @override
  String get theProjectCouldNotBeDeleted =>
      'Impossibile eliminare il progetto.';

  @override
  String get itIsDeletedOnEveryPaired =>
      'Viene eliminato su tutti i dispositivi associati.';

  @override
  String get theFileCouldNotBeDeleted => 'Impossibile eliminare il file.';

  @override
  String get theMeetingCouldNotBeRemoved =>
      'Impossibile togliere la riunione dal progetto.';

  @override
  String get project => 'Progetto';

  @override
  String get searchThisProject => 'Cerca in questo progetto';

  @override
  String get chatAboutThisProject => 'Chatta su questo progetto';

  @override
  String get addFiles => 'Aggiungi file';

  @override
  String get writeANote => 'Scrivi una nota';

  @override
  String get addMeetings => 'Aggiungi riunioni';

  @override
  String get deleteProject => 'Elimina progetto';

  @override
  String get addThemHereOrFromA =>
      'Aggiungile qui, o dalla pagina di una riunione.';

  @override
  String get removeFromProject => 'Togli dal progetto';

  @override
  String get filesNotesAndEMails => 'File, note ed e-mail';

  @override
  String get nothingAddedYet => 'Ancora nulla di aggiunto';

  @override
  String get addPicturesMarkdownPdfsEMails =>
      'Aggiungi immagini, Markdown, PDF, e-mail (.eml) o documenti.';

  @override
  String get onItsWayFromAPaired => 'In arrivo da un dispositivo associato';

  @override
  String get newNote => 'Nuova nota';

  @override
  String get noteMarkdown => 'Nota (Markdown)';

  @override
  String get everyMeetingIsAlreadyInA =>
      'Tutte le riunioni sono già in un progetto.';

  @override
  String get add => 'Aggiungi';

  @override
  String get moveToProject => 'Sposta nel progetto';

  @override
  String get noProject => 'Nessun progetto';

  @override
  String get newProject => 'Nuovo progetto…';

  @override
  String get newProject2 => 'Nuovo progetto';

  @override
  String get create => 'Crea';

  @override
  String get theProjectCouldNotBeCreated => 'Impossibile creare il progetto.';

  @override
  String get theMeetingCouldNotBeMoved => 'Impossibile spostare la riunione.';

  @override
  String get removedFromItsProject => 'Tolta dal suo progetto';

  @override
  String movedTo(Object project) {
    return 'Spostata in $project';
  }

  @override
  String get searchProjects => 'Cerca nei progetti';

  @override
  String get noProjectsYetAProjectGathers =>
      'Ancora nessun progetto. Un progetto raccoglie le riunioni con i file, le note e le e-mail che le riguardano.';

  @override
  String get theRecordingIsStillStartingTry =>
      'La registrazione si sta ancora avviando; riprova.';

  @override
  String recording(Object title) {
    return 'Registrazione di \"$title\"';
  }

  @override
  String get recording2 => 'Registrazione in corso';

  @override
  String paused(Object title) {
    return '\"$title\" in pausa';
  }

  @override
  String get paused2 => 'In pausa';

  @override
  String get finishingTranscription => 'Completamento della trascrizione…';

  @override
  String get readyToRecord => 'Pronto a registrare';

  @override
  String get meetingTitle => 'Titolo della riunione';

  @override
  String get enterMeetingTitleOptional =>
      'Inserisci il titolo della riunione (facoltativo)';

  @override
  String get leaveEmptyToNameTheMeeting =>
      'Lascia vuoto per chiamare la riunione con l\'ora d\'inizio';

  @override
  String get startingRecording => 'Avvio della registrazione';

  @override
  String get startRecording => 'Avvia registrazione';

  @override
  String get pause => 'Pausa';

  @override
  String get resume => 'Riprendi';

  @override
  String get stop => 'Ferma';

  @override
  String get language => 'Lingua';

  @override
  String get aChangeAppliesToWhatIs =>
      'Una modifica vale per ciò che viene detto dopo';

  @override
  String get liveTranscript => 'Trascrizione in diretta';

  @override
  String get theTranscriptAppearsHereAFew =>
      'La trascrizione compare qui pochi secondi dopo ogni pausa nel parlato.';

  @override
  String get nearbyDevicesSeeThisNameWhile =>
      'I dispositivi vicini vedono questo nome mentre questo dispositivo è rilevabile. I dispositivi già associati tengono il nome che conoscono.';

  @override
  String get meetingRenamed => 'Riunione rinominata';

  @override
  String get thisTitleIsnTAllowed => 'Questo titolo non è consentito.';

  @override
  String get couldnTSaveTheNewTitle =>
      'Impossibile salvare il nuovo titolo. Riprova.';

  @override
  String get theNoteTakingEngineIsnT =>
      'Il motore di Koalino non è disponibile in questo momento.';

  @override
  String get thisMeetingNoLongerExists2 => 'Questa riunione non esiste più.';

  @override
  String get couldnTRenameTheMeeting => 'Impossibile rinominare la riunione.';

  @override
  String get thisFileIsStillOnIts =>
      'Questo file è ancora in arrivo da un dispositivo associato.';

  @override
  String get thisPictureCannotBeShown =>
      'Impossibile mostrare questa immagine.';

  @override
  String get thisKindOfDocumentCanT =>
      'Questo tipo di documento non si può ancora mostrare nell\'app.';

  @override
  String get thisFileCannotBeOpened => 'Impossibile aprire questo file.';

  @override
  String get thisFileCannotBeRead => 'Impossibile leggere questo file.';

  @override
  String get thisEMailCannotBeRead => 'Impossibile leggere questa e-mail.';

  @override
  String get noSubject => '(nessun oggetto)';

  @override
  String get from => 'Da';

  @override
  String get to => 'A';

  @override
  String get cc => 'Cc';

  @override
  String get sent => 'Inviata';

  @override
  String attachments(Object attachmentNames) {
    return 'Allegati: $attachmentNames';
  }

  @override
  String get theRestOfThisEMail =>
      'Il resto di questa e-mail è troppo lungo da mostrare.';

  @override
  String get searching => 'Ricerca…';

  @override
  String get nothingFound => 'Nessun risultato.';

  @override
  String get search2 => 'Cerca';

  @override
  String get pairYourDevicesAndSeeWhich =>
      'Associa i tuoi dispositivi e vedi quali sono connessi.';

  @override
  String get transcriptionModel => 'Modello di trascrizione';

  @override
  String get chooseTheWhisperModel => 'Scegli il modello Whisper.';

  @override
  String get languageModelsForMeetingSummariesAnd =>
      'Modelli linguistici per i riassunti delle riunioni, e il prompt del riassunto.';

  @override
  String get keepProjectsAndMeetingsAsFiles =>
      'Tieni progetti e riunioni come file modificabili con qualsiasi editor.';

  @override
  String get letYourOwnAssistantSuchAs =>
      'Consenti al tuo assistente, come Claude Code, di leggere le tue note.';

  @override
  String get appLock => 'Blocco app';

  @override
  String get defaultLanguage => 'Lingua predefinita';

  @override
  String get usedForNewRecordingsAutoDetects =>
      'Usata per le nuove registrazioni. Auto rileva la lingua mentre le persone parlano.';

  @override
  String get transcribeOn => 'Trascrivi su';

  @override
  String get anotherDeviceReceivesTheRecordingS =>
      'Un altro dispositivo riceve l\'audio della registrazione, cifrato, e lo elimina dopo averlo trascritto. Se non è disponibile, trascrive questo dispositivo.';

  @override
  String get telemetry => 'Telemetria';

  @override
  String get howLongTranscriptionTakesOnEach =>
      'Quanto tempo richiede la trascrizione su ogni dispositivo, e il consumo energetico stimato.';

  @override
  String get logLevel => 'Livello dei log';

  @override
  String get howMuchDetailTheAppWrites =>
      'Quanti dettagli l\'app scrive nei log. Debug e Trace aiutano a diagnosticare i problemi ma scrivono di più.';

  @override
  String get custom => 'Personalizzato';

  @override
  String get shareLogs => 'Condividi i log';

  @override
  String get theLast7DaysOfDiagnostic =>
      'Gli ultimi 7 giorni di log diagnostici, per esempio da allegare a una segnalazione di bug.';

  @override
  String get unableToReadTheAppLock =>
      'Impossibile leggere l\'impostazione del blocco app.';

  @override
  String get needsAndroid10OrLater => 'Richiede Android 10 o successivo.';

  @override
  String get setAScreenLockInAndroid =>
      'Imposta un blocco schermo nelle impostazioni di Android per usare il Blocco app.';

  @override
  String askForYourFingerprintFaceOr(Object lockAfterBackground) {
    return 'Chiedi impronta, volto o blocco schermo quando torni su Koalino dopo $lockAfterBackground secondi di assenza. Registrazione e sincronizzazione continuano mentre è bloccato.';
  }

  @override
  String get otherNetworks => 'Altre reti';

  @override
  String get aRelayOnYourOwnAlways =>
      'Un relay su un tuo dispositivo sempre acceso permette ai tuoi dispositivi di raggiungersi anche lontano dalla stessa Wi-Fi. La sincronizzazione sulla stessa rete funziona anche senza.';

  @override
  String get relayAddress => 'Indirizzo del relay';

  @override
  String get deviceAddressesOptionalOnePerLine =>
      'Indirizzi dei dispositivi (facoltativi, uno per riga)';

  @override
  String get saveAddresses => 'Salva indirizzi';

  @override
  String get nothingThatWasSharedCouldBe =>
      'Nulla di ciò che è stato condiviso si è potuto leggere.';

  @override
  String couldNotBeRead(Object message, Object failedCount) {
    return '$message; $failedCount non leggibili';
  }

  @override
  String saveAsTheSummaryOf(Object title) {
    return 'Salva come riassunto di \"$title\"';
  }

  @override
  String get addToAProjectInstead => 'Aggiungi invece a un progetto';

  @override
  String get splitThisMeeting => 'Separare questa riunione?';

  @override
  String itWasRecordedOnAndThe(Object devices, Object recordings) {
    return 'È stata registrata su $devices, e l\'app ha unito le registrazioni. Separandola, l\'altra registrazione ($recordings) torna a parte, con la propria trascrizione, qui e sui dispositivi associati. Non verranno più unite. Questa riunione mantiene la sua trascrizione.';
  }

  @override
  String get unableToSplitThisMeeting =>
      'Impossibile separare questa riunione.';

  @override
  String get telemetryDashboard => 'Pannello della telemetria';

  @override
  String get transcription => 'Trascrizione';

  @override
  String get latency => 'Latenza';

  @override
  String get tasks => 'Attività';

  @override
  String get power => 'Energia';

  @override
  String get transcriptionDurationOverTime =>
      'Durata della trascrizione nel tempo';

  @override
  String get latencyDistribution => 'Distribuzione della latenza';

  @override
  String get taskTypeBreakdown => 'Ripartizione per tipo di attività';

  @override
  String get powerConsumptionEstimate => 'Stima del consumo energetico';

  @override
  String get fastestLeastAccurate => 'Il più veloce, il meno preciso';

  @override
  String get fastManyMistakesTheAppShips =>
      'Veloce, molti errori. L\'app include questo';

  @override
  String get goodBalanceOnMostComputers =>
      'Buon equilibrio sulla maggior parte dei computer';

  @override
  String get accurateSlowerNeedsAbout2Gb =>
      'Preciso, più lento, richiede circa 2 GB di memoria';

  @override
  String get olderNeedsAPowerfulComputer =>
      'Meno recente, richiede un computer potente';

  @override
  String get mostAccurateNeedsAPowerfulComputer =>
      'Il più preciso, richiede un computer potente';

  @override
  String get veryAccurateBestWithAGraphics =>
      'Molto preciso. Ideale con una scheda grafica';

  @override
  String get thisModelMayBeTooLarge =>
      'Questo modello potrebbe essere troppo grande';

  @override
  String mayNeedMoreMemoryThanThis(Object model) {
    return '$model potrebbe richiedere più memoria di quella di questo dispositivo, e le registrazioni potrebbero fallire o essere lente.';
  }

  @override
  String get downloadAnyway => 'Scarica comunque';

  @override
  String get transcriptionSettingsAreUnavailable =>
      'Le impostazioni di trascrizione non sono disponibili.';

  @override
  String get downloadFromQualcomm => 'Scaricare da Qualcomm?';

  @override
  String get about360MbIsDownloadedFrom =>
      'Vengono scaricati circa 360 MB da Qualcomm, che occupano circa 460 MB:\n\n• Il runtime QNN di Qualcomm, che permette alle app di usare la NPU. È concesso con la AI Stack License di Qualcomm, non con la licenza dell\'app.\n• Whisper Small, compilato da Qualcomm AI Hub per la NPU di questo telefono.\n\nOgni file viene verificato prima dell\'uso. Le tue registrazioni non lasciano mai il telefono. Scaricando, accetti i termini di Qualcomm.';

  @override
  String get acceptAndDownload => 'Accetta e scarica';

  @override
  String get downloadingTheNpuFiles => 'Download dei file per la NPU…';

  @override
  String get transcribeOnTheNpu => 'Trascrivi sulla NPU';

  @override
  String get thisPhoneHasAnAiChip =>
      'Questo telefono ha un chip per l\'IA (NPU) che può eseguire Whisper Small lasciando libero il processore: 30 secondi di parlato richiedono circa 2 secondi. Serve un download da Qualcomm.';

  @override
  String get download => 'Scarica';

  @override
  String get runsWhisperSmallOnThePhone =>
      'Esegue Whisper Small sul chip IA del telefono, al posto del modello scelto sotto, anche per la passata dopo la riunione se questa non ha un modello suo. I tempi delle parole sono approssimativi.';

  @override
  String get removeTheNpuFiles => 'Rimuovi i file per la NPU';

  @override
  String get alsoRecordTheComputerSSound =>
      'Registra anche l\'audio del computer';

  @override
  String get transcribesWhatTheComputerPlaysSuch =>
      'Trascrive ciò che il computer riproduce, come l\'altra parte di una chiamata, accanto al microfono. Quando il microfono sente anche gli altoparlanti, le parole ripetute vengono tenute una sola volta.';

  @override
  String get improveTheTranscriptAfterTheMeeting =>
      'Migliora la trascrizione dopo la riunione';

  @override
  String get transcribesEachMeetingAgainWithMore =>
      'Ritrascrive ogni riunione con più contesto quando finisce; un telefono aspetta di essere in carica. Fino ad allora il parlato resta cifrato, poi viene eliminato.';

  @override
  String get modelForThisPass => 'Modello per questa passata';

  @override
  String get aLargerModelIsSlowerBut =>
      'Un modello più grande è più lento ma più preciso, e dopo la riunione c\'è tempo. Scarica i modelli qui sopra.';

  @override
  String get sameAsForRecording => 'Lo stesso della registrazione';

  @override
  String get keepSpeechForCombining => 'Conserva il parlato per combinarlo';

  @override
  String get whenAMeetingWasAlsoRecorded =>
      'Quando una riunione è stata registrata anche su un altro dispositivo, il suo parlato attende per questo tempo di essere combinato, poi viene eliminato.';

  @override
  String get whisperModel => 'Modello Whisper';

  @override
  String get biggerModelsMakeFewerMistakesAnd =>
      'I modelli più grandi sbagliano meno, ma richiedono più tempo e memoria. I download provengono da Hugging Face. Questa scelta vale solo per questo dispositivo.';

  @override
  String automatic2(Object fileName, Object mode) {
    return 'Automatico: $fileName$mode';
  }

  @override
  String usesUntilIsDownloadedTheBest(
      Object fileName, Object waiting, Object reason) {
    return 'Usa $fileName finché non viene scaricato $waiting, il più adatto a questo dispositivo. $reason';
  }

  @override
  String picksTheBestModelForThis(Object fileName, Object mode) {
    return 'Sceglie il modello migliore per questo dispositivo: $fileName$mode.';
  }

  @override
  String download2(Object sizeBytes) {
    return 'Scarica $sizeBytes';
  }

  @override
  String tryLabel(Object model) {
    return 'Prova $model';
  }

  @override
  String get use => 'Usa';

  @override
  String suggestedForThisDevice(Object model, Object mode) {
    return 'Consigliato per questo dispositivo: $model$mode';
  }

  @override
  String get efficientMode => 'Modalità efficiente';

  @override
  String runsTheSameModelsOnThe(Object gpuName) {
    return 'Esegue gli stessi modelli sul chip grafico ($gpuName), di solito più velocemente. Disattivata usa il processore, come di consueto.';
  }

  @override
  String get theGraphicsChipStoppedTheApp =>
      'L\'ultima volta il chip grafico ha bloccato l\'app, quindi questo dispositivo trascrive con il processore.';

  @override
  String get tryTheGraphicsChipAgain => 'Riprova il chip grafico';

  @override
  String get onThisDevice => 'su questo dispositivo';

  @override
  String get usedByAutomatic => 'usato da Automatico';

  @override
  String get englishOnly => 'Solo inglese';

  @override
  String get mayBeTooLargeForThis =>
      'potrebbe essere troppo grande per questo dispositivo';

  @override
  String get doesnTMatchYourLanguage => 'non corrisponde alla tua lingua';

  @override
  String get notEnoughFreeSpace => 'spazio libero insufficiente';

  @override
  String get cancelDownload => 'Annulla download';

  @override
  String get glossary => 'Glossario';

  @override
  String get namesProjectNamesAndTermsOne =>
      'Nomi, nomi di progetti e termini, uno per riga. La trascrizione li scrive come qui.';

  @override
  String get saveGlossary => 'Salva glossario';

  @override
  String get voiceGallery => 'Galleria voci';

  @override
  String get tryAgain => 'Riprova';

  @override
  String get unknownVoices => 'Voci sconosciute';

  @override
  String get noUnknownSpeakersToReview =>
      'Nessuna voce sconosciuta da rivedere';

  @override
  String get savedPeople => 'Persone salvate';

  @override
  String get noSavedPeopleYetNameA =>
      'Ancora nessuna persona salvata. Dai un nome a una voce per aggiungere qualcuno.';

  @override
  String delete4(Object displayName) {
    return 'Elimina $displayName';
  }

  @override
  String get playing => 'Riproduzione…';

  @override
  String get untitledMeeting => 'Riunione senza titolo';

  @override
  String get play => 'Riproduci';

  @override
  String get deleteThisVoice => 'Elimina questa voce';

  @override
  String theCallShowedSpeakingCheckThen(Object suggestedName) {
    return 'La chiamata mostrava $suggestedName mentre parlava. Verifica, poi Assegna.';
  }

  @override
  String get chooseASavedPerson => 'Scegli una persona salvata';

  @override
  String get assign => 'Assegna';

  @override
  String get deleteTheseVoices => 'Elimina queste voci';

  @override
  String get theseSoundLikeOnePersonUntick =>
      'Sembrano una sola persona. Togli la spunta a quelle di qualcun altro.';

  @override
  String itsVoiceSamplesGoForGood(Object voices) {
    return 'I suoi $voices campioni vocali vengono eliminati definitivamente, qui e sui dispositivi associati. Le trascrizioni restano.';
  }

  @override
  String get itsVoiceSampleGoesForGood =>
      'Il suo campione vocale viene eliminato definitivamente, qui e sui dispositivi associati. La trascrizione resta.';

  @override
  String get theirNameAndVoiceSamplesGo =>
      'Il suo nome e i suoi campioni vocali vengono eliminati definitivamente, qui e sui dispositivi associati. Le trascrizioni restano e lo numerano come voce.';

  @override
  String unknownSpeaker(Object index) {
    return 'Voce sconosciuta $index';
  }

  @override
  String attachedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count file allegati',
      one: '1 file allegato',
    );
    return '$_temp0';
  }

  @override
  String addedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count file aggiunti',
      one: '1 file aggiunto',
    );
    return '$_temp0';
  }

  @override
  String emailAttachmentsLeft(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count allegati sono rimasti nell\'e-mail',
      one: '1 allegato è rimasto nell\'e-mail',
    );
    return '$_temp0';
  }

  @override
  String addedMeetings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count riunioni aggiunte',
      one: '1 riunione aggiunta',
    );
    return '$_temp0';
  }

  @override
  String otherRecordings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count registrazioni',
      one: '1 registrazione',
    );
    return '$_temp0';
  }

  @override
  String get severalDevices => 'più dispositivi';

  @override
  String meetingCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count riunioni',
      one: '1 riunione',
    );
    return '$_temp0';
  }

  @override
  String fileCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count file',
      one: '1 file',
    );
    return '$_temp0';
  }

  @override
  String chunkCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count frammenti',
      one: '1 frammento',
    );
    return '$_temp0';
  }

  @override
  String resultCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count risultati',
      one: '1 risultato',
    );
    return '$_temp0';
  }

  @override
  String get moreNotShown =>
      ', altri non mostrati: aggiungi parole per restringere';

  @override
  String cpuCores(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count core CPU',
      one: '1 core CPU',
    );
    return '$_temp0';
  }

  @override
  String dayCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count giorni',
      one: '1 giorno',
    );
    return '$_temp0';
  }

  @override
  String voiceSampleCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count campioni vocali',
      one: '1 campione vocale',
      zero: 'nessun campione vocale ancora, quindi non riconosciuto dalla voce',
    );
    return '$_temp0';
  }

  @override
  String picturesIncluded(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count immagini incluse',
      one: '1 immagine inclusa',
    );
    return '$_temp0';
  }

  @override
  String partCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count parti',
      one: '1 parte',
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
      other: '$count voci assegnate a $name',
      one: 'Assegnato a $name',
    );
    return '$_temp0';
  }

  @override
  String addSharedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Aggiungi $count file condivisi a un progetto',
      one: 'Aggiungi il file condiviso a un progetto',
    );
    return '$_temp0';
  }

  @override
  String keepSpeechSaved(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other:
          'Salvato. Ora il parlato attende fino a $count giorni per essere combinato.',
      one:
          'Salvato. Ora il parlato attende fino a 1 giorno per essere combinato.',
    );
    return '$_temp0';
  }

  @override
  String titlesCanBeAtMost(Object max) {
    return 'I titoli possono avere al massimo $max caratteri.';
  }

  @override
  String promptUnknownPlaceholder(Object example) {
    return 'Il prompt contiene un $example sconosciuto o non chiuso.';
  }

  @override
  String get live => '● In diretta';

  @override
  String get offline => '○ Offline';

  @override
  String minutesShort(Object minutes) {
    return '$minutes min';
  }

  @override
  String get appLanguage => 'Lingua dell\'app';

  @override
  String get appLanguageDescription =>
      'La lingua dei menu e dei messaggi di Koalino.';

  @override
  String get systemLanguage => 'Predefinita di sistema';

  @override
  String photosAddedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count foto aggiunte',
      one: '1 foto aggiunta',
    );
    return '$_temp0';
  }

  @override
  String moreModels(int count) {
    return 'Altri modelli ($count)';
  }

  @override
  String get moreModelsHint =>
      'Versioni più leggere che sbagliano un po\' di più, versioni solo in inglese e modelli che potrebbero non entrare in questo dispositivo.';

  @override
  String get microphoneSilencedByCall =>
      'Android sta silenziando il microfono mentre un\'altra app lo usa per una chiamata: non viene registrato nulla finché la chiamata non termina.';

  @override
  String get handoffInTheBrowser =>
      'Koalino apre il sito nel browser, dove hai effettuato l\'accesso con il tuo account, e gli passa la riunione con il tuo prompt. La finestra successiva mostra cosa fare lì.';

  @override
  String toKeepTheSummaryCopyTheWebsite(String appName) {
    return 'Per conservare il riassunto, copia la risposta di $appName sul sito e torna su Koalino.';
  }

  @override
  String get useYourClaudeOrChatgptPlan => 'Usa il tuo piano Claude o ChatGPT';

  @override
  String get summariesCanUseYourOwnPlan =>
      'I riassunti possono usare il tuo abbonamento Claude o ChatGPT, senza chiave API. Installa Claude Code o Codex e accedi una volta: poi Koalino scrive i riassunti da solo.';

  @override
  String get installClaudeCode => 'Installa Claude Code';

  @override
  String get forAClaudePlanOpensTheGuide =>
      'Per un piano Claude. Apre la guida all\'installazione.';

  @override
  String get installCodex => 'Installa Codex';

  @override
  String get forAChatgptPlanOpensTheGuide =>
      'Per un piano ChatGPT. Apre la guida all\'installazione.';

  @override
  String get skipUseTheWebsite => 'Salta: usa il sito';

  @override
  String get koalinoCopiesTheMeetingAndOpens =>
      'Niente da installare: Koalino copia la riunione e apre claude.ai o chatgpt.com, e tu la incolli lì.';

  @override
  String get addAConnection => 'Aggiungi una connessione';

  @override
  String get claudeCodeOrCodexAlreadyInstalled =>
      'Claude Code o Codex già installati, o un server nella tua rete.';

  @override
  String get noConnectionNeededUseTheWebsite =>
      'Non serve una connessione. In una riunione apri Riassunto e scegli Usa la tua app di IA…: Koalino copia la riunione e apre claude.ai o chatgpt.com.';

  @override
  String get afterInstallingAddTheConnection =>
      'Dopo averlo installato ed effettuato l\'accesso, torna qui e scegli Aggiungi connessione → Aggiungi una connessione → Claude Code o Codex.';

  @override
  String get claudeAiOrChatgptComInYourBrowser =>
      'claude.ai o chatgpt.com nel browser, con il tuo account';

  @override
  String get newMeeting => 'Nuova riunione';

  @override
  String get newMeetingExplanation =>
      'Una riunione senza registrazione. Scrivi note o aggiungi file: riassunti e chat li usano.';

  @override
  String get titleOptional => 'Titolo (facoltativo)';

  @override
  String handoffStepPressEnter(String site) {
    return '$site si apre con la riunione già nella casella del messaggio. Premi Invio per inviarla.';
  }

  @override
  String handoffStepSentByItself(String site) {
    return '$site si apre e invia la riunione da solo. Se lo chiede, accedi prima.';
  }

  @override
  String handoffStepPaste(String site) {
    return '$site si apre e Koalino copia la riunione. Fai clic nella casella del messaggio, incollala (Ctrl+V) e premi Invio.';
  }

  @override
  String get handoffStepDragFiles =>
      'Si apre anche una cartella con i file: trascinali tutti nella casella del messaggio, così vanno come file.';

  @override
  String get handoffStepCopyAnswer =>
      'Quando la risposta è completa, premi il pulsante di copia sotto di essa.';

  @override
  String get handoffStepComeBack =>
      'Torna su Koalino: la risposta diventa il riassunto della riunione, con Annulla.';

  @override
  String get handoffStepAskThere => 'Poi fai lì le tue domande sulla riunione.';

  @override
  String openSite(String site) {
    return 'Apri $site';
  }

  @override
  String writtenBySourceOn(String source, String createdAt) {
    return 'Scritto da $source il $createdAt';
  }

  @override
  String get noRecordingWriteNotesOrAttach =>
      'Nessuna registrazione. Scrivi note o allega file: riassunti e chat li usano.';

  @override
  String handoffStepOpensFilled(String site) {
    return '$site si apre con la riunione già nella casella del messaggio.';
  }

  @override
  String handoffStepOpensCopied(String site) {
    return '$site si apre e Koalino copia la riunione.';
  }

  @override
  String get handoffStepSend => 'Premi Invio per inviarla.';

  @override
  String get handoffStepPasteSend =>
      'Fai clic nella casella del messaggio, incolla la riunione (Ctrl+V) e premi Invio.';

  @override
  String get chatHandoffIntro =>
      'Fai le tue domande nella tua app di IA, con il tuo account. Koalino le passa note e trascrizioni, e foto e file vanno insieme come file.';

  @override
  String get plainTextFile => 'Testo semplice (.txt)';

  @override
  String get unableToExportTheSummary => 'Impossibile esportare il riassunto';
}
