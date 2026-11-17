// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for French (`fr`).
class AppLocalizationsFr extends AppLocalizations {
  AppLocalizationsFr([String locale = 'fr']) : super(locale);

  @override
  String get unableToCheckTheAppLock =>
      'Impossible de vérifier le verrouillage de l\'app. Réessayez.';

  @override
  String get unableToShowTheUnlockPrompt =>
      'Impossible d\'afficher la demande de déverrouillage. Réessayez.';

  @override
  String get setAScreenLockOnThis =>
      'Configurez d\'abord un verrouillage d\'écran sur cet appareil.';

  @override
  String get unableToChangeTheAppLock =>
      'Impossible de modifier le verrouillage de l\'app. Réessayez.';

  @override
  String get aCallApp => 'Une app d\'appel';

  @override
  String get call => 'Appel';

  @override
  String call2(Object app) {
    return 'Appel $app';
  }

  @override
  String theCallEndedSoTheRecording(Object tiedApp) {
    return 'L\'appel $tiedApp est terminé, l\'enregistrement s\'est donc arrêté.';
  }

  @override
  String get restartYourBrowserAndCallApps =>
      'Redémarrez une fois votre navigateur et vos apps d\'appel, pour que Koalino puisse lire qui parle pendant les appels.';

  @override
  String get unableToLoadDevices => 'Impossible de charger les appareils';

  @override
  String get thisDeviceIsNotOnA =>
      'Cet appareil n\'est pas encore sur un réseau. Connectez-le et réessayez.';

  @override
  String get thatDidNotWorkTryAgain => 'Cela n\'a pas fonctionné. Réessayez.';

  @override
  String get use1To64CharactersWithout =>
      'Utilisez de 1 à 64 caractères, sans saut de ligne.';

  @override
  String get theNameCouldNotBeSaved =>
      'Impossible d\'enregistrer le nom. Réessayez.';

  @override
  String get thisInviteHasExpiredMakeA =>
      'Cette invitation a expiré. Créez-en une nouvelle sur l\'autre appareil.';

  @override
  String get thisInviteWasMadeOnThis =>
      'Cette invitation a été créée sur cet appareil. Utilisez-la sur l\'autre.';

  @override
  String get thatDeviceIsAlreadyPaired => 'Cet appareil est déjà associé.';

  @override
  String get thatIsNotAValidInvite =>
      'Ce n\'est pas une invitation valide. Copiez-la à nouveau.';

  @override
  String get deviceSyncIsNotRunning =>
      'La synchronisation des appareils n\'est pas active.';

  @override
  String get theOtherDeviceIsNoLonger =>
      'L\'autre appareil n\'est plus en cours d\'association. Réessayez.';

  @override
  String get theOtherAppCouldNotBe => 'Impossible d\'ouvrir l\'autre app.';

  @override
  String get theOtherApp => 'l\'autre app';

  @override
  String app(Object displayName) {
    return 'app $displayName';
  }

  @override
  String get unableToChangeTheLogLevel =>
      'Impossible de modifier le niveau de journalisation';

  @override
  String logsExportedTo(Object filePath) {
    return 'Journaux exportés vers $filePath';
  }

  @override
  String get unableToShareTheLogs => 'Impossible de partager les journaux';

  @override
  String get noLogsToExportYet => 'Aucun journal à exporter pour l\'instant';

  @override
  String get unableToExportTheLogs => 'Impossible d\'exporter les journaux';

  @override
  String couldNotBeAttached(Object attached, Object failedPaths) {
    return '$attached ; $failedPaths n\'ont pas pu être joints';
  }

  @override
  String get theFilesCouldNotBeAttached =>
      'Impossible de joindre les fichiers.';

  @override
  String get unableToShareTheExport => 'Impossible de partager l\'export';

  @override
  String exportedTo(Object filePath) {
    return 'Exporté vers $filePath';
  }

  @override
  String get exportSaved => 'Export enregistré';

  @override
  String get unableToSaveTheExport => 'Impossible d\'enregistrer l\'export';

  @override
  String get thisMeetingNoLongerExists => 'Cette réunion n\'existe plus';

  @override
  String get exportIsUnavailableUntilTheApp =>
      'L\'export est indisponible tant que l\'app n\'a pas démarré';

  @override
  String get unableToExportTheMeeting => 'Impossible d\'exporter la réunion';

  @override
  String get unableToLoadMeetings => 'Impossible de charger les réunions';

  @override
  String get unableToLoadThisMeeting => 'Impossible de charger cette réunion';

  @override
  String get enterATitle => 'Saisissez un titre.';

  @override
  String titlesCanBeAtMostCharacters(
      Object maxMeetingTitleCharacters, Object runes) {
    return 'Les titres peuvent compter au plus $maxMeetingTitleCharacters caractères (vous en avez $runes).';
  }

  @override
  String get titlesCanTContainLineBreaks =>
      'Les titres ne peuvent pas contenir de sauts de ligne ni de caractères de contrôle.';

  @override
  String get notPairedYetMakeTheRelay =>
      'Pas encore associé. Rendez le relais visible, puis associez-le sous Appareils à proximité dans Appareils.';

  @override
  String get connectingToTheRelay => 'Connexion au relais…';

  @override
  String get connectedToTheRelayAskingIt =>
      'Connecté au relais. Demande d\'acceptation des connexions…';

  @override
  String get connectedYourOtherDevicesCanReach =>
      'Connecté. Vos autres appareils peuvent joindre celui-ci via le relais.';

  @override
  String get savedThisAppliesNowWithNo =>
      'Enregistré. C\'est effectif tout de suite, sans redémarrage.';

  @override
  String get thatRelayAddressIsNotValid =>
      'Cette adresse de relais n\'est pas valide. Elle ressemble à /ip4/203.0.113.5/tcp/4001/p2p/12D3Koo…';

  @override
  String get aDeviceAddressIsNotValid =>
      'Une adresse d\'appareil n\'est pas valide. Chacune se termine par /p2p/ et l\'ID de l\'appareil.';

  @override
  String get unableToSaveTheAddresses =>
      'Impossible d\'enregistrer les adresses.';

  @override
  String get thePhotoCouldNotBeSaved => 'Impossible d\'enregistrer la photo.';

  @override
  String get enterAName => 'Saisissez un nom.';

  @override
  String namesCanBeAtMostCharacters(Object maxMeetingTitleCharacters) {
    return 'Les noms peuvent compter au plus $maxMeetingTitleCharacters caractères.';
  }

  @override
  String get namesCanTContainLineBreaks =>
      'Les noms ne peuvent pas contenir de sauts de ligne ni de caractères de contrôle.';

  @override
  String get unableToLoadProjects => 'Impossible de charger les projets';

  @override
  String get unableToLoadThisProject => 'Impossible de charger ce projet';

  @override
  String get theFilesCouldNotBeAdded => 'Impossible d\'ajouter les fichiers.';

  @override
  String couldNotBeAdded(Object failedPaths) {
    return '$failedPaths n\'ont pas pu être ajoutés';
  }

  @override
  String get unableToStartRecording =>
      'Impossible de démarrer l\'enregistrement';

  @override
  String get unableToPauseRecording =>
      'Impossible de mettre l\'enregistrement en pause';

  @override
  String get unableToResumeRecording =>
      'Impossible de reprendre l\'enregistrement';

  @override
  String get unableToStopRecording => 'Impossible d\'arrêter l\'enregistrement';

  @override
  String get recordingSaved => 'Enregistrement sauvegardé';

  @override
  String get microphoneAccessIsNeededToRecord =>
      'L\'accès au micro est nécessaire pour enregistrer';

  @override
  String get keepTheAppOpenAndroidDid =>
      'Gardez l\'app ouverte : Android n\'a pas autorisé l\'enregistrement en arrière-plan';

  @override
  String get unableToOpenTheMicrophone => 'Impossible d\'ouvrir le micro';

  @override
  String saved(Object label) {
    return '$label enregistrée.';
  }

  @override
  String removed(Object label) {
    return '$label supprimée.';
  }

  @override
  String isNowTheDefault(Object id) {
    return '$id est désormais la connexion par défaut.';
  }

  @override
  String answered(Object id, Object reply) {
    return '$id a répondu : $reply';
  }

  @override
  String get promptSaved => 'Prompt enregistré.';

  @override
  String get theBuiltInPromptIsBack => 'Le prompt intégré est rétabli.';

  @override
  String get finishedMeetingsWillBeSummarizedAutomatically =>
      'Les réunions terminées seront résumées automatiquement.';

  @override
  String get automaticSummariesAreOff =>
      'Les résumés automatiques sont désactivés.';

  @override
  String get thisComputerNoLongerWritesSummaries =>
      'Cet ordinateur ne rédige plus de résumés et ne répond plus aux chats pour vos autres appareils.';

  @override
  String nowWritesTheSummariesAndAnswers(Object id) {
    return '$id rédige désormais les résumés et répond aux chats demandés par vos autres appareils.';
  }

  @override
  String get requestCancelled => 'Demande annulée.';

  @override
  String get thisMeetingUsesTheSettingsPrompt =>
      'Cette réunion utilise à nouveau le prompt des Paramètres.';

  @override
  String get promptSavedForThisMeeting =>
      'Prompt enregistré pour cette réunion.';

  @override
  String get summarySaved => 'Résumé enregistré.';

  @override
  String get theConnection => 'La connexion';

  @override
  String get summaryWritten => 'Résumé rédigé.';

  @override
  String theTranscriptWasReadInParts(Object parts) {
    return ' La transcription a été lue en $parts parties.';
  }

  @override
  String leftOut(Object sent, Object imagesSkipped) {
    return ' $sent, $imagesSkipped laissées de côté.';
  }

  @override
  String get theModelStoppedBeforeTheEnd =>
      ' Le modèle s\'est arrêté avant la fin.';

  @override
  String get waitingForYourComputerItWrites =>
      'En attente de votre ordinateur. Il rédigera le résumé une fois allumé, connecté à cet appareil et configuré pour répondre dans Paramètres → Connexions IA.';

  @override
  String get yourComputerIsWritingTheSummary =>
      'Votre ordinateur rédige le résumé…';

  @override
  String yourComputerCouldNotWriteThe(Object message) {
    return 'Votre ordinateur n\'a pas pu rédiger le résumé : $message';
  }

  @override
  String get unableToRefreshTelemetry =>
      'Impossible d\'actualiser la télémétrie';

  @override
  String get automatic => 'Automatique';

  @override
  String get transcriptionLanguage => 'Langue de transcription';

  @override
  String get transcriptionLanguages => 'Langues de transcription';

  @override
  String get theLanguageCouldNotBeChanged => 'Impossible de changer la langue.';

  @override
  String get thisDevice => 'Cet appareil';

  @override
  String get bestAvailable => 'Le meilleur disponible';

  @override
  String get aRemovedDevice => 'Un appareil supprimé';

  @override
  String get theTranscriptionDeviceCouldNotBe =>
      'Impossible de changer l\'appareil de transcription. Vérifiez qu\'il est toujours associé.';

  @override
  String get downloadFinishedYouCanNowChoose =>
      'Téléchargement terminé. Vous pouvez maintenant choisir ce modèle.';

  @override
  String get downloadCancelled => 'Téléchargement annulé.';

  @override
  String get theDownloadStoppedNoConnectionTo =>
      'Le téléchargement s\'est arrêté : pas de connexion au serveur. Votre modèle actuel fonctionne toujours.';

  @override
  String get theDownloadedFileFailedItsCheck =>
      'Le fichier téléchargé n\'a pas passé la vérification et a été supprimé. Votre modèle actuel fonctionne toujours.';

  @override
  String get theModelCouldNotBeSaved =>
      'Impossible d\'enregistrer le modèle. Vérifiez l\'espace libre. Votre modèle actuel fonctionne toujours.';

  @override
  String get theDownloadServerRefusedTheFile =>
      'Le serveur de téléchargement a refusé le fichier. Votre modèle actuel fonctionne toujours.';

  @override
  String get savedTheNextRecordingUsesThis =>
      'Enregistré. Le prochain enregistrement utilisera ce modèle.';

  @override
  String get savedTheNextRecordingUsesThis2 =>
      'Enregistré. Le prochain enregistrement l\'utilisera.';

  @override
  String get savedTheNextRecordingUsesThe =>
      'Enregistré. Le prochain enregistrement utilisera le choix automatique.';

  @override
  String get savedMeetingsRecordedFromNowOn =>
      'Enregistré. Les réunions enregistrées désormais l\'utiliseront.';

  @override
  String get savedTheNextRecordingUsesThese =>
      'Enregistré. Le prochain enregistrement utilisera ces noms et termes.';

  @override
  String get eachTermCanHaveAtMost =>
      'Chaque terme peut compter au plus 60 caractères, et l\'ensemble au plus 500.';

  @override
  String get chooseAWindowBetween1And =>
      'Choisissez une durée entre 1 et 60 jours.';

  @override
  String get theDownloadCouldNotStartPlease =>
      'Le téléchargement n\'a pas pu démarrer. Réessayez.';

  @override
  String get savedTheNextRecordingIsTranscribed =>
      'Enregistré. Le prochain enregistrement sera transcrit sur le NPU.';

  @override
  String get savedTheNextRecordingIsTranscribed2 =>
      'Enregistré. Le prochain enregistrement sera transcrit sur le processeur.';

  @override
  String get theNpuFilesAreNotAll =>
      'Les fichiers du NPU ne sont pas encore tous téléchargés.';

  @override
  String get theNpuFilesWereRemoved => 'Les fichiers du NPU ont été supprimés.';

  @override
  String get theNpuFilesCouldNotBe =>
      'Impossible de supprimer les fichiers du NPU. Réessayez.';

  @override
  String get downloadedRecordingsNowTranscribeOnThe =>
      'Téléchargé. Les enregistrements sont désormais transcrits sur le NPU.';

  @override
  String get theDownloadStoppedNoConnectionTo2 =>
      'Le téléchargement s\'est arrêté : pas de connexion au serveur.';

  @override
  String get aDownloadedFileFailedItsCheck =>
      'Un fichier téléchargé n\'a pas passé la vérification et a été supprimé.';

  @override
  String get theFilesCouldNotBeSaved =>
      'Impossible d\'enregistrer les fichiers. Vérifiez l\'espace libre (environ 800 Mo nécessaires).';

  @override
  String get theDownloadServerRefusedTheFiles =>
      'Le serveur de téléchargement a refusé les fichiers.';

  @override
  String get theNextRecordingTriesTheGraphics =>
      'Le prochain enregistrement réessaiera la puce graphique.';

  @override
  String get thatModelWonTBeProposed => 'Ce modèle ne sera plus proposé.';

  @override
  String get modelRemoved => 'Modèle supprimé.';

  @override
  String get anotherModelIsDownloadingWaitFor =>
      'Un autre modèle est en cours de téléchargement. Attendez qu\'il se termine.';

  @override
  String get thereIsnTEnoughFreeSpace =>
      'Il n\'y a pas assez d\'espace libre pour ce modèle.';

  @override
  String get thatDidnTWorkPleaseTry => 'Cela n\'a pas fonctionné. Réessayez.';

  @override
  String get unableToLoadVoiceGallery =>
      'Impossible de charger la Galerie des voix';

  @override
  String get unableToPlayThisVoice => 'Impossible de lire cette voix';

  @override
  String get unableToDeleteThisVoice => 'Impossible de supprimer cette voix';

  @override
  String get voiceDeleted => 'Voix supprimée';

  @override
  String get unableToDeleteThisPerson =>
      'Impossible de supprimer cette personne';

  @override
  String deleted(Object displayName) {
    return '$displayName supprimé';
  }

  @override
  String get enterANameBeforeAssigning => 'Saisissez un nom avant d\'attribuer';

  @override
  String assignedTo(Object identityName) {
    return 'Attribué à $identityName';
  }

  @override
  String assignedToThereWasNotEnough(Object identityName) {
    return 'Attribué à $identityName. Il n\'y avait pas assez de parole claire pour reconnaître cette voix dans les prochains enregistrements.';
  }

  @override
  String get unableToAssignTheSpeakerCluster =>
      'Impossible d\'attribuer le groupe d\'interventions';

  @override
  String get chooseAtLeastOneVoiceTo => 'Choisissez au moins une voix à nommer';

  @override
  String thereWasNotEnoughClearSpeech(Object assigned) {
    return '$assigned. Il n\'y avait pas assez de parole claire pour reconnaître cette voix dans les prochains enregistrements.';
  }

  @override
  String get theSettingCouldNotBeSaved =>
      'Impossible d\'enregistrer le réglage.';

  @override
  String get theAppIsStillStartingTry =>
      'L\'app est encore en cours de démarrage. Réessayez.';

  @override
  String get somethingWentWrongTryAgain =>
      'Une erreur s\'est produite. Réessayez.';

  @override
  String isUsingTheMicrophone(Object app) {
    return '$app utilise le micro';
  }

  @override
  String get recordThisCall => 'Enregistrer cet appel ?';

  @override
  String theAiServerRejectedTheRequest(Object detail) {
    return 'Le serveur IA a refusé la demande.\nLe serveur a répondu : $detail';
  }

  @override
  String claudeCodeOrCodexCouldNot(Object detail) {
    return 'Claude Code ou Codex n\'a pas pu répondre.\nIl a répondu : $detail';
  }

  @override
  String get typeAQuestion => 'Saisissez une question.';

  @override
  String get theQuestionIsTooLong => 'La question est trop longue.';

  @override
  String get addAnAiConnectionInSettings =>
      'Ajoutez une connexion IA dans les Paramètres, ou choisissez-en une par défaut.';

  @override
  String get thatAiConnectionNoLongerExists =>
      'Cette connexion IA n\'existe plus.';

  @override
  String get thereIsNothingToTalkAbout =>
      'Il n\'y a encore rien à discuter : ni transcription, ni résumé, ni notes, ni fichiers.';

  @override
  String get theAiConnectionSContextIs =>
      'Le contexte de la connexion IA est trop petit. Choisissez-en une avec un contexte plus grand.';

  @override
  String get thatMeetingNoLongerExists => 'Cette réunion n\'existe plus.';

  @override
  String get thatProjectNoLongerExists => 'Ce projet n\'existe plus.';

  @override
  String get theAiServerCouldNotBe =>
      'Impossible de joindre le serveur IA. Vérifiez la connexion.';

  @override
  String get theAiServerRefusedTheApi => 'Le serveur IA a refusé la clé API.';

  @override
  String get theAiServerIsBusyTry =>
      'Le serveur IA est occupé. Réessayez plus tard.';

  @override
  String get theModelDeclinedToAnswer => 'Le modèle a refusé de répondre.';

  @override
  String get claudeCodeOrCodexIsNot =>
      'Claude Code ou Codex n\'est pas installé sur cet ordinateur, ou l\'app ne le trouve pas.';

  @override
  String get theNotesCouldNotBeRead => 'Impossible de lire les notes.';

  @override
  String get noPairedComputerAnsweredOnA =>
      'Aucun ordinateur associé n\'a répondu. Sur un ordinateur, ouvrez Paramètres → IA et choisissez une connexion sous « Résumés pour mes autres appareils », puis laissez-le allumé et connecté.';

  @override
  String get theComputerTookTooLongTo =>
      'L\'ordinateur a mis trop de temps à répondre.';

  @override
  String get theComputerDisconnectedBeforeAnsweringTry =>
      'L\'ordinateur s\'est déconnecté avant de répondre. Réessayez.';

  @override
  String get theComputerAnswersAtMost30 =>
      'L\'ordinateur répond à 30 questions par heure au maximum. Réessayez plus tard.';

  @override
  String get theComputerIsAnsweringOtherQuestions =>
      'L\'ordinateur répond à d\'autres questions. Réessayez dans un instant.';

  @override
  String get pairedDevicesAreOffOnThis =>
      'Les appareils associés sont désactivés sur ce téléphone.';

  @override
  String get theAiServerCouldNotAnswer =>
      'Le serveur IA n\'a pas pu répondre. Réessayez.';

  @override
  String get exportMeeting => 'Exporter la réunion';

  @override
  String get openaiCompatibleServer => 'Serveur compatible OpenAI';

  @override
  String get localServerOllama => 'Serveur local (Ollama)';

  @override
  String get claudeCodeClaudePlan => 'Claude Code (abonnement Claude)';

  @override
  String get codexChatgptPlan => 'Codex (abonnement ChatGPT)';

  @override
  String get checkTheAddressAndModelPlain =>
      'Vérifiez l\'adresse et le modèle. Le http:// simple ne fonctionne que sur cet appareil ou sur le réseau local.';

  @override
  String get thatConnectionNoLongerExists => 'Cette connexion n\'existe plus.';

  @override
  String get addAnAiConnectionInSettings2 =>
      'Ajoutez d\'abord une connexion IA dans les Paramètres.';

  @override
  String get thisMeetingHasNoTranscriptOr =>
      'Cette réunion n\'a pas encore de transcription ni de notes.';

  @override
  String get thisMeetingIsTooLongFor =>
      'Cette réunion est trop longue pour cette connexion. Choisissez-en une avec un contexte plus grand.';

  @override
  String get theAiServerCouldNotBe2 => 'Impossible de joindre le serveur IA.';

  @override
  String get theAiServerRefusedAccessUse =>
      'Le serveur IA a refusé l\'accès. Utilisez un serveur qui ne demande pas de clé API.';

  @override
  String get theAiServerIsBusyOr =>
      'Le serveur IA est occupé ou a dépassé sa limite. Réessayez plus tard.';

  @override
  String get theAiServerRejectedTheRequest2 =>
      'Le serveur IA a refusé la demande. Vérifiez le nom du modèle.';

  @override
  String get theAiServerFailedTryAgain => 'Le serveur IA a échoué. Réessayez.';

  @override
  String get theAiServerSAnswerCould =>
      'Impossible de lire la réponse du serveur IA.';

  @override
  String get claudeCodeOrCodexCouldNot2 =>
      'Claude Code ou Codex n\'a pas pu répondre. Vérifiez qu\'il est connecté.';

  @override
  String get theSettingsCouldNotBeSaved =>
      'Impossible d\'enregistrer les paramètres.';

  @override
  String get theMeetingCouldNotBeRead =>
      'Impossible de lire ou d\'enregistrer la réunion.';

  @override
  String get error => 'Erreur';

  @override
  String get warning => 'Avertissement';

  @override
  String get infoDefault => 'Info (par défaut)';

  @override
  String get debug => 'Débogage';

  @override
  String get trace => 'Trace';

  @override
  String get thisDevice2 => 'cet appareil';

  @override
  String get anotherDevice => 'un autre appareil';

  @override
  String notConnected(Object label) {
    return '$label (non connecté)';
  }

  @override
  String itIsKeptEncryptedUntil(Object until) {
    return ' Il est conservé chiffré jusqu\'au $until.';
  }

  @override
  String get recordedOnThisDeviceOnly =>
      'Enregistrée sur cet appareil uniquement.';

  @override
  String recordedOnThisDeviceOnlyA(Object until) {
    return 'Enregistrée sur cet appareil uniquement. Un enregistrement fait sur un autre appareil peut encore être combiné jusqu\'au $until.';
  }

  @override
  String combinesTheRecordingsNothingIsLeft(Object combiner) {
    return '$combiner combine les enregistrements ; il ne reste rien à envoyer d\'ici.';
  }

  @override
  String waitingForAudioFrom(Object names, Object kept) {
    return 'En attente de l\'audio de $names.$kept';
  }

  @override
  String combinesTheRecordingsWaitingForAudio(Object combiner, Object names) {
    return '$combiner combine les enregistrements ; en attente de l\'audio de $names.';
  }

  @override
  String get audioFromEveryDeviceIsHere =>
      'L\'audio de chaque appareil est arrivé ; la transcription est combinée quand cet appareil est libre.';

  @override
  String thisDeviceSAudioStillHas(Object combiner, Object kept) {
    return 'L\'audio de cet appareil doit encore parvenir à $combiner.$kept';
  }

  @override
  String get transcriptCombinedFromEveryDevice =>
      'Transcription combinée à partir de chaque appareil.';

  @override
  String notCombinedAudioFromDidNot(Object plainNames) {
    return 'Non combinée : l\'audio de $plainNames n\'est pas arrivé dans les trois semaines.';
  }

  @override
  String meeting(Object createdAt) {
    return 'Réunion $createdAt';
  }

  @override
  String get writeSomethingFirst => 'Écrivez d\'abord quelque chose.';

  @override
  String get theTextIsTooLong => 'Le texte est trop long.';

  @override
  String get thatPartOfTheTranscriptChanged =>
      'Cette partie de la transcription a changé entre-temps. Rouvrez la réunion.';

  @override
  String get theMeetingCouldNotBeSaved =>
      'Impossible d\'enregistrer la réunion.';

  @override
  String get chooseAFullFolderPathInside =>
      'Choisissez le chemin complet d\'un dossier, dans votre dossier personnel mais pas le dossier personnel lui-même.';

  @override
  String get theFolderCouldNotBeWritten =>
      'Impossible d\'écrire dans le dossier. Vérifiez qu\'il existe et que vous pouvez y écrire.';

  @override
  String get theNotesCouldNotBeRead2 =>
      'Impossible de lire ou d\'enregistrer les notes.';

  @override
  String get theInviteAnswerIsIncomplete =>
      'La réponse à l\'invitation est incomplète.';

  @override
  String get theRenameAnswerIsIncomplete =>
      'La réponse au changement de nom est incomplète.';

  @override
  String get thisDeviceHasNoCamera => 'Cet appareil n\'a pas de caméra.';

  @override
  String get theCameraIsNotOpen => 'La caméra n\'est pas ouverte.';

  @override
  String get title => 'Titre';

  @override
  String get summary => 'Résumé';

  @override
  String get notes => 'Notes';

  @override
  String get transcript => 'Transcription';

  @override
  String get projectName => 'Nom du projet';

  @override
  String file(Object fileName) {
    return 'Fichier $fileName';
  }

  @override
  String get typeWhatToLookFor => 'Saisissez ce que vous cherchez.';

  @override
  String get typeAtLeastTwoLetters => 'Saisissez au moins deux lettres.';

  @override
  String get theSearchIsTooLong => 'La recherche est trop longue.';

  @override
  String get exportLogs => 'Exporter les journaux';

  @override
  String about(Object appName) {
    return 'À propos de $appName';
  }

  @override
  String get versionLicenseAndTheSoftwareIt =>
      'Version, licence et logiciels utilisés.';

  @override
  String remove(Object label) {
    return 'Supprimer $label ?';
  }

  @override
  String get itsApiKeyIsDeletedFrom =>
      'Sa clé API est effacée de cet appareil. Les résumés déjà rédigés restent.';

  @override
  String get cancel => 'Annuler';

  @override
  String get remove2 => 'Supprimer';

  @override
  String get aiConnections => 'Connexions IA';

  @override
  String get addConnection => 'Ajouter une connexion';

  @override
  String get summariesSendTheMeetingSTranscript =>
      'Les résumés envoient la transcription, les notes et les photos de la réunion au serveur de la connexion. Un serveur sur votre réseau local les garde chez vous.';

  @override
  String get noConnectionsYet => 'Aucune connexion pour l\'instant';

  @override
  String get addAServerOnYourNetwork =>
      'Ajoutez un serveur de votre réseau (Ollama, LM Studio, llama.cpp), ou Claude Code ou Codex sur cet ordinateur.';

  @override
  String get summarizeMeetingsAutomatically =>
      'Résumer automatiquement les réunions';

  @override
  String get withTheDefaultConnectionWhenA =>
      'Avec la connexion par défaut, quand la transcription d\'une réunion est définitive.';

  @override
  String get summariesForMyOtherDevices => 'Résumés pour mes autres appareils';

  @override
  String get aPairedPhoneCanAskThis =>
      'Un téléphone associé peut demander à cet ordinateur de rédiger un résumé ou de répondre à un chat avec cette connexion, à partir de la copie de vos notes sur cet ordinateur. Cela compte dans cet abonnement ; au plus 10 résumés et 30 réponses de chat par heure.';

  @override
  String get off => 'Désactivé';

  @override
  String get summaryPrompt => 'Prompt de résumé';

  @override
  String placeholdersTheTranscriptIsAlwaysSent(Object placeholders) {
    return 'Variables : $placeholders. La transcription est toujours envoyée après le prompt.';
  }

  @override
  String get savePrompt => 'Enregistrer le prompt';

  @override
  String get resetToBuiltIn => 'Rétablir l\'intégré';

  @override
  String get defaultLabel => 'Par défaut';

  @override
  String get installed => 'Installé';

  @override
  String get notFoundOnThisComputer => 'Introuvable sur cet ordinateur';

  @override
  String get unencryptedHttpLocalNetworkOnly =>
      'http non chiffré, réseau local uniquement';

  @override
  String get runsWithYourPlanSSign =>
      'Utilise la connexion de votre abonnement sur cet ordinateur';

  @override
  String actionsFor(Object label) {
    return 'Actions pour $label';
  }

  @override
  String get makeDefault => 'Définir par défaut';

  @override
  String get test => 'Tester';

  @override
  String get edit => 'Modifier';

  @override
  String get enterANameAnAddressAnd =>
      'Saisissez un nom, une adresse et un modèle.';

  @override
  String get theContextIsANumberOf =>
      'Le contexte est un nombre de tokens, par exemple 8192.';

  @override
  String edit2(Object label) {
    return 'Modifier $label';
  }

  @override
  String get name => 'Nom';

  @override
  String get protocol => 'Protocole';

  @override
  String get runsTheToolInstalledAndSigned =>
      'Utilise l\'outil installé et connecté sur cet ordinateur : votre abonnement Claude ou ChatGPT rédige les résumés au lieu du crédit API. La réunion est envoyée à Anthropic ou OpenAI sous ce compte.';

  @override
  String get address => 'Adresse';

  @override
  String get httpsAlwaysHttpOnlyOnThis =>
      'https:// toujours ; http:// uniquement sur cet appareil ou le réseau local.';

  @override
  String get model => 'Modèle';

  @override
  String get leaveEmptyForTheToolS =>
      'Laissez vide pour le modèle par défaut de l\'outil.';

  @override
  String get contextTokens => 'Contexte (tokens)';

  @override
  String get saveConnection => 'Enregistrer la connexion';

  @override
  String get koalinoIsLocked => 'Koalino est verrouillé';

  @override
  String get unlockWithYourFingerprintFaceOr =>
      'Déverrouillez avec votre empreinte, votre visage ou le verrouillage d\'écran.';

  @override
  String get unlock => 'Déverrouiller';

  @override
  String copied(Object what) {
    return '$what copié';
  }

  @override
  String get aiAssistantAccess => 'Accès de l\'assistant IA';

  @override
  String get letYourOwnAiAssistantSuch =>
      'Laissez votre propre assistant IA, comme Claude Code, Codex ou Claude Desktop, lire vos réunions et projets : titres, résumés, notes, transcriptions et fichiers de projet. Il peut seulement lire ; il ne peut rien modifier ni supprimer.';

  @override
  String get whatTheAssistantReadsMayBe =>
      'Ce que lit l\'assistant peut être envoyé à son modèle en ligne, sous votre compte auprès de ce service. N\'activez cette option que pour un assistant à qui vous confieriez vos réunions.';

  @override
  String get theAssistantServerIsNotInstalled =>
      'Le serveur de l\'assistant n\'est pas installé avec cette copie de l\'app.';

  @override
  String get letMyAiAssistantReadMy =>
      'Laisser mon assistant IA lire mes notes';

  @override
  String get connectYourAssistant => 'Connecter votre assistant';

  @override
  String get yourNotesAreUnlockedWithA =>
      'Vos notes sont déverrouillées par une phrase secrète, qui n\'est jamais écrite dans les paramètres d\'un assistant. Le serveur ne les lit que si NOTE_TAKER_ENCRYPTION_PASSPHRASE est définie là où l\'assistant le lance.';

  @override
  String get claudeCodeRunInATerminal =>
      'Claude Code : exécutez dans un terminal';

  @override
  String get command => 'Commande';

  @override
  String get claudeDesktopAndOtherMcpClients =>
      'Claude Desktop et autres clients MCP : ajoutez à leurs paramètres';

  @override
  String get settings => 'Paramètres';

  @override
  String copy(Object title) {
    return 'Copier : $title';
  }

  @override
  String isUsingTheMicrophoneRecordThis(Object app) {
    return '$app utilise le micro. Enregistrer cet appel ?';
  }

  @override
  String get notNow => 'Pas maintenant';

  @override
  String get record => 'Enregistrer';

  @override
  String get offerToRecordCalls => 'Proposer d\'enregistrer les appels';

  @override
  String get whenTeamsZoomMeetInA =>
      'Quand Teams, Zoom, Meet dans un navigateur ou une autre app d\'appel commence à utiliser le micro. Rien n\'est enregistré tant que vous n\'appuyez pas sur Enregistrer.';

  @override
  String get stopWhenTheCallEnds => 'Arrêter à la fin de l\'appel';

  @override
  String get onlyForRecordingsStartedFromThe =>
      'Uniquement pour les enregistrements lancés depuis la proposition, une fois que l\'app d\'appel a libéré le micro depuis 20 secondes.';

  @override
  String get readSpeakerNamesFromTheCall =>
      'Lire les noms des intervenants dans la fenêtre d\'appel';

  @override
  String get whileRecordingKoalinoReadsWhoTeams =>
      'Pendant l\'enregistrement, Koalino lit qui Teams, Zoom ou Meet affiche comme intervenant, pour proposer des noms aux voix inconnues. Rien ne quitte cet ordinateur, et personne n\'est nommé tant que vous n\'appuyez pas sur Attribuer.';

  @override
  String get deleteThisChat => 'Supprimer ce chat ?';

  @override
  String get itsQuestionsAndAnswersAreDeleted =>
      'Ses questions et réponses sont effacées de cet appareil.';

  @override
  String get delete => 'Supprimer';

  @override
  String get thatFileIsNoLongerIn => 'Ce fichier n\'est plus dans le projet.';

  @override
  String get whoAnswers => 'Qui répond';

  @override
  String get myPairedComputer => 'Mon ordinateur associé';

  @override
  String get thisPhoneSAiConnection => 'La connexion IA de ce téléphone';

  @override
  String get deleteThisChat2 => 'Supprimer ce chat';

  @override
  String get askAboutYourNotesAnswersCome =>
      'Posez des questions sur vos notes. Les réponses viennent de votre ordinateur associé, qui lit sa propre copie de vos notes avec la connexion choisie là-bas pour vos autres appareils.';

  @override
  String get askAboutYourNotesTheAnswer =>
      'Posez des questions sur vos notes. La réponse vient de votre connexion IA par défaut, à laquelle sont envoyées les notes qui tiennent.';

  @override
  String get yourQuestion => 'Votre question';

  @override
  String get ask => 'Demander';

  @override
  String get open => 'Ouvrir';

  @override
  String given(Object included) {
    return 'Fourni : $included';
  }

  @override
  String leftOutForLength(Object omitted) {
    return 'Laissé de côté pour la longueur : $omitted';
  }

  @override
  String answeredBy(Object connection) {
    return 'Réponse de $connection';
  }

  @override
  String get unableToOpenTheKeychain => 'Impossible d\'ouvrir le trousseau';

  @override
  String get koalinoKeepsTheKeyThatEncrypts =>
      'Koalino conserve dans le trousseau du système la clé qui chiffre vos profils vocaux et l\'identité de l\'appareil. Déverrouillez le trousseau, ou autorisez Koalino à l\'utiliser, puis réessayez. Si cet appareil n\'a pas de service de trousseau, installez-en ou démarrez-en un et redémarrez Koalino.';

  @override
  String get unableToOpenYourNotes => 'Impossible d\'ouvrir vos notes';

  @override
  String get koalinoCouldNotOpenItsDatabase =>
      'Koalino n\'a pas pu ouvrir sa base de données. Vérifiez qu\'il reste de l\'espace libre et réessayez.';

  @override
  String get unlockingYourNotes => 'Déverrouillage de vos notes…';

  @override
  String get quit => 'Quitter';

  @override
  String get retry => 'Réessayer';

  @override
  String delete2(Object title) {
    return 'Supprimer $title ?';
  }

  @override
  String get itsTranscriptAttachmentsAndUnknownVoices =>
      'Sa transcription, ses pièces jointes et ses voix inconnues sont supprimées définitivement, ici et sur vos appareils associés. Les personnes enregistrées restent.';

  @override
  String get unableToDeleteThisMeetingIf =>
      'Impossible de supprimer cette réunion. Si elle est en cours d\'enregistrement, arrêtez d\'abord l\'enregistrement.';

  @override
  String get devices => 'Appareils';

  @override
  String get deviceSyncIsOff =>
      'La synchronisation des appareils est désactivée';

  @override
  String get meetingsStayOnThisDeviceUntil =>
      'Les réunions restent sur cet appareil jusqu\'à l\'activation de la synchronisation.';

  @override
  String thisDevice3(Object capabilities) {
    return 'Cet appareil\n$capabilities';
  }

  @override
  String get renameThisDevice => 'Renommer cet appareil';

  @override
  String get discoverableYourOtherDevicesCanAsk =>
      'Visible : vos autres appareils peuvent demander l\'association pendant quelques minutes.';

  @override
  String get makeDiscoverable => 'Rendre visible';

  @override
  String get pairingRequests => 'Demandes d\'association';

  @override
  String get acceptOnlyIfTheOtherDevice =>
      'N\'acceptez que si l\'autre appareil affiche le même code.';

  @override
  String get reject => 'Refuser';

  @override
  String get accept => 'Accepter';

  @override
  String get pairingInProgress => 'Association en cours';

  @override
  String get waitingForTheOtherDevice => 'En attente de l\'autre appareil…';

  @override
  String get checkThatTheOtherDeviceShows =>
      'Vérifiez que l\'autre appareil affiche le même code.';

  @override
  String get cancelPairing => 'Annuler l\'association';

  @override
  String get codesMatch => 'Les codes correspondent';

  @override
  String get pairedDevices => 'Appareils associés';

  @override
  String get noPairedDevicesYet => 'Aucun appareil associé pour l\'instant.';

  @override
  String paired(Object pairedAt, Object hasRelay) {
    return 'Associé le $pairedAt\n$hasRelay';
  }

  @override
  String remove3(Object deviceName) {
    return 'Supprimer $deviceName';
  }

  @override
  String get nearbyDevices => 'Appareils à proximité';

  @override
  String get pair => 'Associer';

  @override
  String get anotherNetwork => 'Un autre réseau';

  @override
  String get showAnInviteHereAndUse =>
      'Affichez une invitation ici et utilisez-la sur l\'autre appareil. Vous comparerez quand même un code avant l\'association.';

  @override
  String get showInvite => 'Afficher l\'invitation';

  @override
  String get useInvite => 'Utiliser une invitation';

  @override
  String stopsSyncingWithThisDevicePair(Object deviceName) {
    return '$deviceName cesse de se synchroniser avec cet appareil. Associez-le à nouveau pour reprendre.';
  }

  @override
  String pairingCode(Object split) {
    return 'Code d\'association $split';
  }

  @override
  String get noOtherDevicesFoundOnThis =>
      'Aucun autre appareil trouvé sur ce réseau. À proximité ne trouve que les appareils sur le même Wi-Fi. Pour un appareil sur un autre réseau, utilisez une invitation.';

  @override
  String get noOtherDevicesFoundOnThis2 =>
      'Aucun autre appareil trouvé sur ce réseau. À proximité ne trouve que les appareils sur le même Wi-Fi. Pour un appareil sur un autre réseau, utilisez une invitation ou définissez un relais dans les Paramètres.';

  @override
  String get notConnectedItReconnectsWhenIt =>
      'Non connecté. Il se reconnecte lorsqu\'il est en ligne et peut joindre votre relais.';

  @override
  String get notConnectedPutBothDevicesOn =>
      'Non connecté. Mettez les deux appareils sur le même Wi-Fi, ou définissez un relais dans les Paramètres.';

  @override
  String get whisperOnDevice => 'Whisper (sur l\'appareil)';

  @override
  String get openaiCloud => 'OpenAI (cloud)';

  @override
  String gbGpu(Object round) {
    return 'GPU $round Go';
  }

  @override
  String mbGpu(Object gpuMegabytes) {
    return 'GPU $gpuMegabytes Mo';
  }

  @override
  String get noTranscriptionModels => 'Aucun modèle de transcription';

  @override
  String get useYourOwnAiApp => 'Utiliser votre propre app d\'IA';

  @override
  String get theMeetingItsNotesAndIts =>
      'La réunion, ses notes et ses fichiers vont dans l\'app que vous choisissez, avec votre propre compte. Pour un résumé, copiez-y la réponse et revenez, ou partagez-la avec Koalino : Koalino la conserve.';

  @override
  String summarizeIn(Object displayName) {
    return 'Résumer dans $displayName';
  }

  @override
  String chatIn(Object displayName) {
    return 'Discuter dans $displayName';
  }

  @override
  String get summarizeInAnotherApp => 'Résumer dans une autre app…';

  @override
  String get chatInAnotherApp => 'Discuter dans une autre app…';

  @override
  String summaryFromSavedTo(Object appName, Object title) {
    return 'Résumé de $appName enregistré dans « $title ».';
  }

  @override
  String get undo => 'Annuler';

  @override
  String get theEarlierSummaryIsBack => 'Le résumé précédent est rétabli.';

  @override
  String toKeepTheSummaryCopyS(Object appName) {
    return 'Pour garder le résumé, copiez la réponse de $appName et revenez dans Koalino, ou partagez la réponse avec Koalino.';
  }

  @override
  String get meetings => 'Réunions';

  @override
  String get projects => 'Projets';

  @override
  String get voices => 'Voix';

  @override
  String get meeting2 => 'Réunion';

  @override
  String get chatAboutThisMeeting => 'Discuter de cette réunion';

  @override
  String chat(Object meeting) {
    return 'Chat : $meeting';
  }

  @override
  String get thisMeeting => 'cette réunion';

  @override
  String get takePhotos => 'Prendre des photos';

  @override
  String get attachFiles => 'Joindre des fichiers';

  @override
  String get export => 'Exporter';

  @override
  String shareAs(Object label) {
    return 'Partager en $label';
  }

  @override
  String get nameSpeakers => 'Nommer les intervenants';

  @override
  String get selectAMeetingToSeeIts =>
      'Sélectionnez une réunion pour voir sa transcription.';

  @override
  String get renameMeeting => 'Renommer la réunion';

  @override
  String get deleteMeeting => 'Supprimer la réunion';

  @override
  String recordedOn(Object devices) {
    return 'Enregistrée sur $devices';
  }

  @override
  String get split => 'Séparer';

  @override
  String get longPressOrRightClickA =>
      'Appuyez longuement ou faites un clic droit sur un passage pour le corriger.';

  @override
  String get noTranscriptIsAvailable => 'Aucune transcription disponible.';

  @override
  String get theSpeakerIsAGuess => 'L\'intervenant est une supposition';

  @override
  String get peopleTalkedAtOnceHereWords =>
      'Plusieurs personnes parlaient en même temps ici ; des mots peuvent manquer';

  @override
  String get overlappingSpeech => 'Paroles superposées';

  @override
  String get someWordsWereUnclear => 'Certains mots étaient peu clairs';

  @override
  String get addedAfterTheMeeting => 'Ajouté après la réunion';

  @override
  String photoOpenFullScreen(Object fileName) {
    return 'Photo $fileName, ouvrir en plein écran';
  }

  @override
  String get save => 'Enregistrer';

  @override
  String get preview => 'Aperçu';

  @override
  String get editNotes => 'Modifier les notes';

  @override
  String get writeNotes => 'Écrire des notes';

  @override
  String get loading => 'Chargement…';

  @override
  String get noNotesYet => 'Aucune note pour l\'instant.';

  @override
  String textAt(Object startTimeMs) {
    return 'Texte à $startTimeMs';
  }

  @override
  String get removedThisTranscriptWonTBe =>
      'Supprimé. Cette transcription ne sera pas retranscrite après la réunion.';

  @override
  String get savedThisTranscriptWonTBe =>
      'Enregistré. Cette transcription ne sera pas retranscrite après la réunion.';

  @override
  String get editText => 'Modifier le texte';

  @override
  String get editTheTranscript => 'Modifier la transcription';

  @override
  String get leaveItEmptyToRemoveThis =>
      'Laissez vide pour supprimer ce passage.';

  @override
  String get searchMeetings => 'Rechercher dans les réunions';

  @override
  String get chatWithAllYourNotes => 'Discuter avec toutes vos notes';

  @override
  String get chatWithAllNotes => 'Discuter avec toutes les notes';

  @override
  String get refresh => 'Actualiser';

  @override
  String get noMeetingsYet => 'Aucune réunion pour l\'instant';

  @override
  String get recordingsYouStartAppearHere =>
      'Les enregistrements que vous lancez apparaissent ici.';

  @override
  String get rename => 'Renommer';

  @override
  String get addToProject => 'Ajouter à un projet…';

  @override
  String get summary2 => 'Résumé…';

  @override
  String get addToProject2 => 'Ajouter à un projet';

  @override
  String writingTheSummaryOf(Object title) {
    return 'Rédaction du résumé de $title';
  }

  @override
  String summaryOf(Object title) {
    return 'Résumé de $title';
  }

  @override
  String summarizeWith(Object label) {
    return 'Résumer avec $label';
  }

  @override
  String defaultLabel2(Object model) {
    return '$model · par défaut';
  }

  @override
  String get summarizeOnMyComputer => 'Résumer sur mon ordinateur';

  @override
  String get withClaudeCodeCodexOrThe =>
      'Avec Claude Code, Codex ou la connexion configurée là-bas';

  @override
  String get useYourOwnAiApp2 => 'Utiliser votre propre app d\'IA…';

  @override
  String get claudeChatgptOrAnotherAppWith =>
      'Claude, ChatGPT ou une autre app, avec votre compte';

  @override
  String get openSummary => 'Ouvrir le résumé';

  @override
  String get editPromptForThisMeeting => 'Modifier le prompt de cette réunion…';

  @override
  String get useTheSettingsPrompt => 'Utiliser le prompt des Paramètres';

  @override
  String get promptForThisMeeting => 'Prompt de cette réunion';

  @override
  String placeholders(Object placeholders) {
    return 'Variables : $placeholders';
  }

  @override
  String get addAnAiConnectionInSettings3 =>
      'Ajoutez une connexion IA dans les Paramètres pour rédiger des résumés.';

  @override
  String get summarizeWith2 => 'Résumer avec';

  @override
  String get summarize => 'Résumer';

  @override
  String get summarizeAgain => 'Résumer à nouveau';

  @override
  String get editPromptForThisMeeting2 => 'Modifier le prompt de cette réunion';

  @override
  String get writingTheSummaryALocalModel =>
      'Rédaction du résumé… Un modèle local peut prendre quelques minutes.';

  @override
  String get noSummaryYet => 'Pas encore de résumé.';

  @override
  String get writeASummary => 'Rédiger un résumé';

  @override
  String writtenByYouOn(Object createdAt) {
    return 'Rédigé par vous le $createdAt';
  }

  @override
  String writtenByOn(
      Object model, Object connectionLabel, Object createdAt, Object edited) {
    return 'Rédigé par $model ($connectionLabel) le $createdAt$edited';
  }

  @override
  String get editSummary => 'Modifier le résumé';

  @override
  String get tryAgainOnMyComputer => 'Réessayer sur mon ordinateur';

  @override
  String get cancelRequest => 'Annuler la demande';

  @override
  String get notesFolder => 'Dossier de notes';

  @override
  String get keepYourProjectsAndMeetingsAs =>
      'Gardez vos projets et réunions sous forme de simples fichiers dans un dossier, pour les lire et les modifier avec n\'importe quel éditeur. Les modifications de notes.md, summary.md et des fichiers de projet reviennent dans l\'app en moins d\'une demi-minute, et les fichiers déposés dans le dossier d\'un projet ou dans le dossier des pièces jointes d\'une réunion sont ajoutés. transcript.md est réécrit par l\'app : corrigez la transcription dans l\'app.';

  @override
  String get theFolderIsADecryptedCopy =>
      'Le dossier est une copie déchiffrée de vos notes. Quiconque peut le lire peut les lire, et les outils de sauvegarde ou de synchronisation le copieront. Gardez-le sur un disque chiffré, dans votre propre dossier utilisateur.';

  @override
  String get folder => 'Dossier';

  @override
  String get chooseAFolder => 'Choisir un dossier';

  @override
  String get keepTheNotesFolder => 'Tenir le dossier de notes à jour';

  @override
  String get syncNow => 'Synchroniser maintenant';

  @override
  String lastSyncAtFailed(Object at, Object code) {
    return 'La dernière synchronisation à $at a échoué : $code';
  }

  @override
  String lastSyncAtFilesWrittenEdits(
      Object at, Object written, Object imported) {
    return 'Dernière synchronisation à $at : $written fichiers écrits, $imported modifications et fichiers importés.';
  }

  @override
  String get inviteADevice => 'Inviter un appareil';

  @override
  String get pairingInviteQrCode => 'Code QR de l\'invitation d\'association';

  @override
  String onTheOtherDeviceOpenDevices(Object hour, Object minute) {
    return 'Sur l\'autre appareil, ouvrez Appareils, choisissez Utiliser une invitation et collez ce lien. Vous pouvez scanner le code avec une app appareil photo pour le copier. Il fonctionne jusqu\'à $hour:$minute.';
  }

  @override
  String get close => 'Fermer';

  @override
  String get inviteLinkCopied => 'Lien d\'invitation copié';

  @override
  String get copyLink => 'Copier le lien';

  @override
  String get deviceFoundChoosePairUnderNearby =>
      'Appareil trouvé. Choisissez Associer sous Appareils à proximité.';

  @override
  String get useAnInvite => 'Utiliser une invitation';

  @override
  String get inviteLink => 'Lien d\'invitation';

  @override
  String get findDevice => 'Trouver l\'appareil';

  @override
  String get pairWithAnotherDevice => 'Associer un autre appareil ?';

  @override
  String get youOpenedAPairingInviteKoalino =>
      'Vous avez ouvert une invitation d\'association. Koalino va trouver l\'appareil qui l\'affiche, et les deux appareils afficheront un code à comparer avant tout partage.';

  @override
  String get pairingStartedCheckThatBothDevices =>
      'Association lancée. Vérifiez que les deux appareils affichent le même code.';

  @override
  String get done => 'Terminé';

  @override
  String get photosAreAddedToTheMeeting =>
      'Les photos sont ajoutées à la réunion au fur et à mesure.';

  @override
  String get takePhoto => 'Prendre une photo';

  @override
  String get theCameraIsUnavailable => 'La caméra est indisponible.';

  @override
  String search(Object name) {
    return 'Rechercher dans $name';
  }

  @override
  String get noteSaved => 'Note enregistrée';

  @override
  String get theNoteCouldNotBeSaved => 'Impossible d\'enregistrer la note.';

  @override
  String couldNotBeAdded2(Object failed) {
    return '$failed n\'ont pas pu être ajoutées.';
  }

  @override
  String get renameProject => 'Renommer le projet';

  @override
  String get theProjectCouldNotBeRenamed => 'Impossible de renommer le projet.';

  @override
  String delete3(Object project) {
    return 'Supprimer « $project » ?';
  }

  @override
  String get itsFilesNotesAndEMails =>
      'Ses fichiers, notes et e-mails sont supprimés sur chaque appareil associé. Ses réunions sont conservées, hors de tout projet.';

  @override
  String get theProjectCouldNotBeDeleted =>
      'Impossible de supprimer le projet.';

  @override
  String get itIsDeletedOnEveryPaired =>
      'Il est supprimé sur chaque appareil associé.';

  @override
  String get theFileCouldNotBeDeleted => 'Impossible de supprimer le fichier.';

  @override
  String get theMeetingCouldNotBeRemoved =>
      'Impossible de retirer la réunion du projet.';

  @override
  String get project => 'Projet';

  @override
  String get searchThisProject => 'Rechercher dans ce projet';

  @override
  String get chatAboutThisProject => 'Discuter de ce projet';

  @override
  String get addFiles => 'Ajouter des fichiers';

  @override
  String get writeANote => 'Écrire une note';

  @override
  String get addMeetings => 'Ajouter des réunions';

  @override
  String get deleteProject => 'Supprimer le projet';

  @override
  String get addThemHereOrFromA =>
      'Ajoutez-les ici, ou depuis la page d\'une réunion.';

  @override
  String get removeFromProject => 'Retirer du projet';

  @override
  String get filesNotesAndEMails => 'Fichiers, notes et e-mails';

  @override
  String get nothingAddedYet => 'Rien n\'a encore été ajouté';

  @override
  String get addPicturesMarkdownPdfsEMails =>
      'Ajoutez des images, du Markdown, des PDF, des e-mails (.eml) ou des documents.';

  @override
  String get onItsWayFromAPaired => 'En route depuis un appareil associé';

  @override
  String get newNote => 'Nouvelle note';

  @override
  String get noteMarkdown => 'Note (Markdown)';

  @override
  String get everyMeetingIsAlreadyInA =>
      'Toutes les réunions sont déjà dans un projet.';

  @override
  String get add => 'Ajouter';

  @override
  String get moveToProject => 'Déplacer vers le projet';

  @override
  String get noProject => 'Aucun projet';

  @override
  String get newProject => 'Nouveau projet…';

  @override
  String get newProject2 => 'Nouveau projet';

  @override
  String get create => 'Créer';

  @override
  String get theProjectCouldNotBeCreated => 'Impossible de créer le projet.';

  @override
  String get theMeetingCouldNotBeMoved => 'Impossible de déplacer la réunion.';

  @override
  String get removedFromItsProject => 'Retirée de son projet';

  @override
  String movedTo(Object project) {
    return 'Déplacée vers $project';
  }

  @override
  String get searchProjects => 'Rechercher dans les projets';

  @override
  String get noProjectsYetAProjectGathers =>
      'Aucun projet pour l\'instant. Un projet regroupe des réunions avec les fichiers, notes et e-mails qui les concernent.';

  @override
  String get theRecordingIsStillStartingTry =>
      'L\'enregistrement est encore en cours de démarrage ; réessayez.';

  @override
  String recording(Object title) {
    return 'Enregistrement de « $title »';
  }

  @override
  String get recording2 => 'Enregistrement';

  @override
  String paused(Object title) {
    return '« $title » en pause';
  }

  @override
  String get paused2 => 'En pause';

  @override
  String get finishingTranscription => 'Fin de la transcription…';

  @override
  String get readyToRecord => 'Prêt à enregistrer';

  @override
  String get meetingTitle => 'Titre de la réunion';

  @override
  String get enterMeetingTitleOptional =>
      'Saisissez le titre de la réunion (facultatif)';

  @override
  String get leaveEmptyToNameTheMeeting =>
      'Laissez vide pour nommer la réunion d\'après son heure de début';

  @override
  String get startingRecording => 'Démarrage de l\'enregistrement';

  @override
  String get startRecording => 'Démarrer l\'enregistrement';

  @override
  String get pause => 'Pause';

  @override
  String get resume => 'Reprendre';

  @override
  String get stop => 'Arrêter';

  @override
  String get language => 'Langue';

  @override
  String get aChangeAppliesToWhatIs =>
      'Un changement s\'applique à ce qui est dit ensuite';

  @override
  String get liveTranscript => 'Transcription en direct';

  @override
  String get theTranscriptAppearsHereAFew =>
      'La transcription apparaît ici quelques secondes après chaque pause dans la parole.';

  @override
  String get nearbyDevicesSeeThisNameWhile =>
      'Les appareils à proximité voient ce nom tant que cet appareil est visible. Les appareils déjà associés gardent le nom qu\'ils connaissent.';

  @override
  String get meetingRenamed => 'Réunion renommée';

  @override
  String get thisTitleIsnTAllowed => 'Ce titre n\'est pas autorisé.';

  @override
  String get couldnTSaveTheNewTitle =>
      'Impossible d\'enregistrer le nouveau titre. Réessayez.';

  @override
  String get theNoteTakingEngineIsnT =>
      'Le moteur de prise de notes n\'est pas disponible pour le moment.';

  @override
  String get thisMeetingNoLongerExists2 => 'Cette réunion n\'existe plus.';

  @override
  String get couldnTRenameTheMeeting => 'Impossible de renommer la réunion.';

  @override
  String get thisFileIsStillOnIts =>
      'Ce fichier est encore en route depuis un appareil associé.';

  @override
  String get thisPictureCannotBeShown => 'Impossible d\'afficher cette image.';

  @override
  String get thisKindOfDocumentCanT =>
      'Ce type de document ne peut pas encore être affiché dans l\'app.';

  @override
  String get thisFileCannotBeOpened => 'Impossible d\'ouvrir ce fichier.';

  @override
  String get thisFileCannotBeRead => 'Impossible de lire ce fichier.';

  @override
  String get thisEMailCannotBeRead => 'Impossible de lire cet e-mail.';

  @override
  String get noSubject => '(sans objet)';

  @override
  String get from => 'De';

  @override
  String get to => 'À';

  @override
  String get cc => 'Cc';

  @override
  String get sent => 'Envoyé';

  @override
  String attachments(Object attachmentNames) {
    return 'Pièces jointes : $attachmentNames';
  }

  @override
  String get theRestOfThisEMail =>
      'La suite de cet e-mail est trop longue pour être affichée.';

  @override
  String get searching => 'Recherche…';

  @override
  String get nothingFound => 'Aucun résultat.';

  @override
  String get search2 => 'Rechercher';

  @override
  String get pairYourDevicesAndSeeWhich =>
      'Associez vos appareils et voyez lesquels sont connectés.';

  @override
  String get transcriptionModel => 'Modèle de transcription';

  @override
  String get chooseTheWhisperModel => 'Choisissez le modèle Whisper.';

  @override
  String get languageModelsForMeetingSummariesAnd =>
      'Modèles de langage pour les résumés de réunions, et le prompt de résumé.';

  @override
  String get keepProjectsAndMeetingsAsFiles =>
      'Gardez projets et réunions sous forme de fichiers modifiables avec n\'importe quel éditeur.';

  @override
  String get letYourOwnAssistantSuchAs =>
      'Laissez votre propre assistant, comme Claude Code, lire vos notes.';

  @override
  String get appLock => 'Verrouillage de l\'app';

  @override
  String get defaultLanguage => 'Langue par défaut';

  @override
  String get usedForNewRecordingsAutoDetects =>
      'Utilisée pour les nouveaux enregistrements. Auto détecte la langue pendant que les gens parlent.';

  @override
  String get transcribeOn => 'Transcrire sur';

  @override
  String get anotherDeviceReceivesTheRecordingS =>
      'Un autre appareil reçoit l\'audio de l\'enregistrement, chiffré, et le supprime une fois transcrit. S\'il est indisponible, cet appareil transcrit à sa place.';

  @override
  String get telemetry => 'Télémétrie';

  @override
  String get howLongTranscriptionTakesOnEach =>
      'Durée de la transcription sur chaque appareil, et sa consommation d\'énergie estimée.';

  @override
  String get logLevel => 'Niveau de journalisation';

  @override
  String get howMuchDetailTheAppWrites =>
      'Quantité de détails que l\'app écrit dans ses journaux. Débogage et Trace aident à diagnostiquer les problèmes mais écrivent davantage.';

  @override
  String get custom => 'Personnalisé';

  @override
  String get shareLogs => 'Partager les journaux';

  @override
  String get theLast7DaysOfDiagnostic =>
      'Les 7 derniers jours de journaux de diagnostic, par exemple à joindre à un rapport de bug.';

  @override
  String get unableToReadTheAppLock =>
      'Impossible de lire le réglage du verrouillage de l\'app.';

  @override
  String get needsAndroid10OrLater =>
      'Nécessite Android 10 ou version ultérieure.';

  @override
  String get setAScreenLockInAndroid =>
      'Configurez un verrouillage d\'écran dans les paramètres d\'Android pour utiliser le verrouillage de l\'app.';

  @override
  String askForYourFingerprintFaceOr(Object lockAfterBackground) {
    return 'Demander votre empreinte, votre visage ou le verrouillage d\'écran quand vous revenez dans Koalino après $lockAfterBackground secondes d\'absence. L\'enregistrement et la synchronisation continuent pendant le verrouillage.';
  }

  @override
  String get otherNetworks => 'Autres réseaux';

  @override
  String get aRelayOnYourOwnAlways =>
      'Un relais sur l\'un de vos appareils toujours allumés permet à vos appareils de se joindre loin du même Wi-Fi. La synchronisation sur le même réseau fonctionne sans lui.';

  @override
  String get relayAddress => 'Adresse du relais';

  @override
  String get deviceAddressesOptionalOnePerLine =>
      'Adresses des appareils (facultatif, une par ligne)';

  @override
  String get saveAddresses => 'Enregistrer les adresses';

  @override
  String get nothingThatWasSharedCouldBe =>
      'Rien de ce qui a été partagé n\'a pu être lu.';

  @override
  String couldNotBeRead(Object message, Object failedCount) {
    return '$message ; $failedCount n\'ont pas pu être lus';
  }

  @override
  String saveAsTheSummaryOf(Object title) {
    return 'Enregistrer comme résumé de « $title »';
  }

  @override
  String get addToAProjectInstead => 'Ajouter plutôt à un projet';

  @override
  String get splitThisMeeting => 'Séparer cette réunion ?';

  @override
  String itWasRecordedOnAndThe(Object devices, Object recordings) {
    return 'Elle a été enregistrée sur $devices, et l\'app a réuni les enregistrements. La séparer remet l\'autre enregistrement ($recordings) à part, avec sa propre transcription, ici et sur vos appareils associés. Ils ne seront plus réunis. Cette réunion garde sa transcription.';
  }

  @override
  String get unableToSplitThisMeeting => 'Impossible de séparer cette réunion.';

  @override
  String get telemetryDashboard => 'Tableau de bord de télémétrie';

  @override
  String get transcription => 'Transcription';

  @override
  String get latency => 'Latence';

  @override
  String get tasks => 'Tâches';

  @override
  String get power => 'Énergie';

  @override
  String get transcriptionDurationOverTime =>
      'Durée de transcription dans le temps';

  @override
  String get latencyDistribution => 'Répartition de la latence';

  @override
  String get taskTypeBreakdown => 'Répartition par type de tâche';

  @override
  String get powerConsumptionEstimate =>
      'Estimation de la consommation d\'énergie';

  @override
  String get fastestLeastAccurate => 'Le plus rapide, le moins précis';

  @override
  String get fastManyMistakesTheAppShips =>
      'Rapide, beaucoup d\'erreurs. L\'app est livrée avec celui-ci';

  @override
  String get goodBalanceOnMostComputers =>
      'Bon équilibre sur la plupart des ordinateurs';

  @override
  String get accurateSlowerNeedsAbout2Gb =>
      'Précis, plus lent, nécessite environ 2 Go de mémoire';

  @override
  String get olderNeedsAPowerfulComputer =>
      'Plus ancien, nécessite un ordinateur puissant';

  @override
  String get mostAccurateNeedsAPowerfulComputer =>
      'Le plus précis, nécessite un ordinateur puissant';

  @override
  String get veryAccurateBestWithAGraphics =>
      'Très précis. Idéal avec une carte graphique';

  @override
  String get thisModelMayBeTooLarge => 'Ce modèle est peut-être trop gros';

  @override
  String mayNeedMoreMemoryThanThis(Object model) {
    return '$model peut demander plus de mémoire que cet appareil n\'en a, et les enregistrements pourraient échouer ou être lents.';
  }

  @override
  String get downloadAnyway => 'Télécharger quand même';

  @override
  String get transcriptionSettingsAreUnavailable =>
      'Les paramètres de transcription sont indisponibles.';

  @override
  String get downloadFromQualcomm => 'Télécharger depuis Qualcomm ?';

  @override
  String get about360MbIsDownloadedFrom =>
      'Environ 360 Mo sont téléchargés depuis Qualcomm et occupent environ 460 Mo :\n\n• Le runtime QNN de Qualcomm, qui permet aux apps d\'utiliser le NPU. Il est sous la licence AI Stack License de Qualcomm, pas sous celle de l\'app.\n• Whisper Small, compilé par Qualcomm AI Hub pour le NPU de ce téléphone.\n\nChaque fichier est vérifié avant utilisation. Vos enregistrements ne quittent jamais le téléphone. En téléchargeant, vous acceptez les conditions de Qualcomm.';

  @override
  String get acceptAndDownload => 'Accepter et télécharger';

  @override
  String get downloadingTheNpuFiles => 'Téléchargement des fichiers du NPU…';

  @override
  String get transcribeOnTheNpu => 'Transcrire sur le NPU';

  @override
  String get thisPhoneHasAnAiChip =>
      'Ce téléphone a une puce IA (NPU) capable d\'exécuter Whisper Small en laissant le processeur libre : 30 secondes de parole prennent environ 2 secondes. Un téléchargement depuis Qualcomm est nécessaire.';

  @override
  String get download => 'Télécharger';

  @override
  String get runsWhisperSmallOnThePhone =>
      'Exécute Whisper Small sur la puce IA du téléphone, à la place du modèle choisi ci-dessous, et pour la passe après la réunion sauf si elle a son propre modèle. Les horaires des mots sont approximatifs.';

  @override
  String get removeTheNpuFiles => 'Supprimer les fichiers du NPU';

  @override
  String get alsoRecordTheComputerSSound =>
      'Enregistrer aussi le son de l\'ordinateur';

  @override
  String get transcribesWhatTheComputerPlaysSuch =>
      'Transcrit ce que l\'ordinateur diffuse, comme l\'autre côté d\'un appel, à côté du micro. Quand le micro entend aussi les haut-parleurs, les mots répétés ne sont gardés qu\'une fois.';

  @override
  String get improveTheTranscriptAfterTheMeeting =>
      'Améliorer la transcription après la réunion';

  @override
  String get transcribesEachMeetingAgainWithMore =>
      'Retranscrit chaque réunion avec plus de contexte une fois terminée ; un téléphone attend d\'être en charge. La parole est conservée chiffrée jusque-là, puis supprimée.';

  @override
  String get modelForThisPass => 'Modèle pour cette passe';

  @override
  String get aLargerModelIsSlowerBut =>
      'Un modèle plus gros est plus lent mais plus précis, et la passe a le temps après la réunion. Téléchargez des modèles ci-dessus.';

  @override
  String get sameAsForRecording => 'Le même que pour l\'enregistrement';

  @override
  String get keepSpeechForCombining =>
      'Conserver la parole pour la combinaison';

  @override
  String get whenAMeetingWasAlsoRecorded =>
      'Quand une réunion a aussi été enregistrée sur un autre appareil, sa parole attend cette durée pour être combinée, puis est supprimée.';

  @override
  String get whisperModel => 'Modèle Whisper';

  @override
  String get biggerModelsMakeFewerMistakesAnd =>
      'Les modèles plus gros font moins d\'erreurs, mais demandent plus de temps et de mémoire. Les téléchargements viennent de Hugging Face. Ce choix ne s\'applique qu\'à cet appareil.';

  @override
  String automatic2(Object fileName, Object mode) {
    return 'Automatique : $fileName$mode';
  }

  @override
  String usesUntilIsDownloadedTheBest(
      Object fileName, Object waiting, Object reason) {
    return 'Utilise $fileName jusqu\'au téléchargement de $waiting, le plus adapté à cet appareil. $reason';
  }

  @override
  String picksTheBestModelForThis(Object fileName, Object mode) {
    return 'Choisit le meilleur modèle pour cet appareil : $fileName$mode.';
  }

  @override
  String download2(Object sizeBytes) {
    return 'Télécharger $sizeBytes';
  }

  @override
  String tryLabel(Object model) {
    return 'Essayer $model';
  }

  @override
  String get use => 'Utiliser';

  @override
  String suggestedForThisDevice(Object model, Object mode) {
    return 'Conseillé pour cet appareil : $model$mode';
  }

  @override
  String get efficientMode => 'Mode efficace';

  @override
  String runsTheSameModelsOnThe(Object gpuName) {
    return 'Exécute les mêmes modèles sur la puce graphique ($gpuName), généralement plus vite. Désactivé utilise le processeur, comme d\'habitude.';
  }

  @override
  String get theGraphicsChipStoppedTheApp =>
      'La puce graphique a arrêté l\'app la dernière fois, cet appareil transcrit donc avec son processeur.';

  @override
  String get tryTheGraphicsChipAgain => 'Réessayer la puce graphique';

  @override
  String get onThisDevice => 'sur cet appareil';

  @override
  String get usedByAutomatic => 'utilisé par Automatique';

  @override
  String get englishOnly => 'Anglais uniquement';

  @override
  String get mayBeTooLargeForThis => 'peut-être trop gros pour cet appareil';

  @override
  String get doesnTMatchYourLanguage => 'ne correspond pas à votre langue';

  @override
  String get notEnoughFreeSpace => 'espace libre insuffisant';

  @override
  String get cancelDownload => 'Annuler le téléchargement';

  @override
  String get glossary => 'Glossaire';

  @override
  String get namesProjectNamesAndTermsOne =>
      'Noms, noms de projets et termes, un par ligne. La transcription les écrit comme ici.';

  @override
  String get saveGlossary => 'Enregistrer le glossaire';

  @override
  String get voiceGallery => 'Galerie des voix';

  @override
  String get tryAgain => 'Réessayer';

  @override
  String get unknownVoices => 'Voix inconnues';

  @override
  String get noUnknownSpeakersToReview => 'Aucune voix inconnue à examiner';

  @override
  String get savedPeople => 'Personnes enregistrées';

  @override
  String get noSavedPeopleYetNameA =>
      'Aucune personne enregistrée pour l\'instant. Nommez une voix pour ajouter quelqu\'un.';

  @override
  String delete4(Object displayName) {
    return 'Supprimer $displayName';
  }

  @override
  String get playing => 'Lecture…';

  @override
  String get untitledMeeting => 'Réunion sans titre';

  @override
  String get play => 'Lire';

  @override
  String get deleteThisVoice => 'Supprimer cette voix';

  @override
  String theCallShowedSpeakingCheckThen(Object suggestedName) {
    return 'L\'appel montrait $suggestedName en train de parler. Vérifiez, puis Attribuer.';
  }

  @override
  String get chooseASavedPerson => 'Choisir une personne enregistrée';

  @override
  String get assign => 'Attribuer';

  @override
  String get deleteTheseVoices => 'Supprimer ces voix';

  @override
  String get theseSoundLikeOnePersonUntick =>
      'Elles semblent être une seule personne. Décochez celles de quelqu\'un d\'autre.';

  @override
  String itsVoiceSamplesGoForGood(Object voices) {
    return 'Ses $voices échantillons de voix sont supprimés définitivement, ici et sur vos appareils associés. Les transcriptions restent.';
  }

  @override
  String get itsVoiceSampleGoesForGood =>
      'Son échantillon de voix est supprimé définitivement, ici et sur vos appareils associés. La transcription reste.';

  @override
  String get theirNameAndVoiceSamplesGo =>
      'Son nom et ses échantillons de voix sont supprimés définitivement, ici et sur vos appareils associés. Les transcriptions restent et le numérotent comme intervenant.';

  @override
  String unknownSpeaker(Object index) {
    return 'Intervenant inconnu $index';
  }

  @override
  String attachedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count fichiers joints',
      one: '1 fichier joint',
    );
    return '$_temp0';
  }

  @override
  String addedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count fichiers ajoutés',
      one: '1 fichier ajouté',
    );
    return '$_temp0';
  }

  @override
  String emailAttachmentsLeft(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count pièces jointes sont restées dans l\'e-mail',
      one: '1 pièce jointe est restée dans l\'e-mail',
    );
    return '$_temp0';
  }

  @override
  String addedMeetings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count réunions ajoutées',
      one: '1 réunion ajoutée',
    );
    return '$_temp0';
  }

  @override
  String otherRecordings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count enregistrements',
      one: '1 enregistrement',
    );
    return '$_temp0';
  }

  @override
  String get severalDevices => 'plusieurs appareils';

  @override
  String meetingCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count réunions',
      one: '1 réunion',
    );
    return '$_temp0';
  }

  @override
  String fileCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count fichiers',
      one: '1 fichier',
    );
    return '$_temp0';
  }

  @override
  String chunkCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count extraits',
      one: '1 extrait',
    );
    return '$_temp0';
  }

  @override
  String resultCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count résultats',
      one: '1 résultat',
    );
    return '$_temp0';
  }

  @override
  String get moreNotShown =>
      ', d\'autres non affichés : ajoutez des mots pour affiner';

  @override
  String cpuCores(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count cœurs CPU',
      one: '1 cœur CPU',
    );
    return '$_temp0';
  }

  @override
  String dayCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count jours',
      one: '1 jour',
    );
    return '$_temp0';
  }

  @override
  String voiceSampleCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count échantillons de voix',
      one: '1 échantillon de voix',
      zero: 'pas encore d\'échantillon de voix, donc pas reconnu à la voix',
    );
    return '$_temp0';
  }

  @override
  String picturesIncluded(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count images incluses',
      one: '1 image incluse',
    );
    return '$_temp0';
  }

  @override
  String partCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count parties',
      one: '1 partie',
    );
    return '$_temp0';
  }

  @override
  String inMeetings(Object parts, Object meetings) {
    return '$parts dans $meetings';
  }

  @override
  String assignedVoicesTo(int count, Object name) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count voix attribuées à $name',
      one: 'Attribué à $name',
    );
    return '$_temp0';
  }

  @override
  String addSharedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Ajouter $count fichiers partagés à un projet',
      one: 'Ajouter le fichier partagé à un projet',
    );
    return '$_temp0';
  }

  @override
  String keepSpeechSaved(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other:
          'Enregistré. La parole attend désormais jusqu\'à $count jours pour être combinée.',
      one:
          'Enregistré. La parole attend désormais jusqu\'à 1 jour pour être combinée.',
    );
    return '$_temp0';
  }

  @override
  String titlesCanBeAtMost(Object max) {
    return 'Les titres peuvent compter au plus $max caractères.';
  }

  @override
  String promptUnknownPlaceholder(Object example) {
    return 'Le prompt contient une variable $example inconnue ou non fermée.';
  }

  @override
  String get live => '● En direct';

  @override
  String get offline => '○ Hors ligne';

  @override
  String minutesShort(Object minutes) {
    return '$minutes min';
  }

  @override
  String get appLanguage => 'Langue de l\'app';

  @override
  String get appLanguageDescription =>
      'La langue des menus et des messages de Koalino.';

  @override
  String get systemLanguage => 'Langue du système';

  @override
  String photosAddedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count photos ajoutées',
      one: '1 photo ajoutée',
    );
    return '$_temp0';
  }

  @override
  String moreModels(int count) {
    return 'Autres modèles ($count)';
  }

  @override
  String get moreModelsHint =>
      'Versions plus légères qui font un peu plus d\'erreurs, versions en anglais uniquement, et modèles qui risquent de ne pas tenir sur cet appareil.';

  @override
  String get microphoneSilencedByCall =>
      'Android coupe le micro tant qu\'une autre app l\'utilise pour un appel : rien n\'est enregistré avant la fin de l\'appel.';

  @override
  String get handoffInTheBrowser =>
      'Koalino ouvre le site dans votre navigateur, où vous êtes connecté avec votre propre compte, et lui transmet la réunion avec votre prompt. La fenêtre suivante indique quoi faire là-bas.';

  @override
  String toKeepTheSummaryCopyTheWebsite(String appName) {
    return 'Pour garder le résumé, copiez la réponse de $appName sur le site et revenez dans Koalino.';
  }

  @override
  String get useYourClaudeOrChatgptPlan =>
      'Utiliser votre forfait Claude ou ChatGPT';

  @override
  String get summariesCanUseYourOwnPlan =>
      'Les résumés peuvent utiliser votre propre abonnement Claude ou ChatGPT, sans clé API. Installez Claude Code ou Codex et connectez-vous une fois : Koalino écrit ensuite les résumés tout seul.';

  @override
  String get installClaudeCode => 'Installer Claude Code';

  @override
  String get forAClaudePlanOpensTheGuide =>
      'Pour un forfait Claude. Ouvre le guide d\'installation.';

  @override
  String get installCodex => 'Installer Codex';

  @override
  String get forAChatgptPlanOpensTheGuide =>
      'Pour un forfait ChatGPT. Ouvre le guide d\'installation.';

  @override
  String get skipUseTheWebsite => 'Passer : utiliser le site';

  @override
  String get koalinoCopiesTheMeetingAndOpens =>
      'Rien à installer : Koalino copie la réunion et ouvre claude.ai ou chatgpt.com, et vous la collez là.';

  @override
  String get addAConnection => 'Ajouter une connexion';

  @override
  String get claudeCodeOrCodexAlreadyInstalled =>
      'Claude Code ou Codex déjà installé, ou un serveur de votre réseau.';

  @override
  String get noConnectionNeededUseTheWebsite =>
      'Aucune connexion nécessaire. Dans une réunion, ouvrez Résumé et choisissez Utiliser votre propre app d\'IA… : Koalino copie la réunion et ouvre claude.ai ou chatgpt.com.';

  @override
  String get afterInstallingAddTheConnection =>
      'Après l\'installation et la connexion, revenez et choisissez Ajouter une connexion → Ajouter une connexion → Claude Code ou Codex.';

  @override
  String get claudeAiOrChatgptComInYourBrowser =>
      'claude.ai ou chatgpt.com dans votre navigateur, avec votre compte';

  @override
  String get newMeeting => 'Nouvelle réunion';

  @override
  String get newMeetingExplanation =>
      'Une réunion sans enregistrement. Écrivez des notes ou ajoutez des fichiers : les résumés et le chat les utilisent.';

  @override
  String get titleOptional => 'Titre (facultatif)';

  @override
  String handoffStepPressEnter(String site) {
    return '$site s\'ouvre avec la réunion déjà dans la zone de message. Appuyez sur Entrée pour l\'envoyer.';
  }

  @override
  String handoffStepSentByItself(String site) {
    return '$site s\'ouvre et envoie la réunion tout seul. Connectez-vous d\'abord s\'il le demande.';
  }

  @override
  String handoffStepPaste(String site) {
    return '$site s\'ouvre et Koalino copie la réunion. Cliquez dans la zone de message, collez-la (Ctrl+V) et appuyez sur Entrée.';
  }

  @override
  String get handoffStepDragFiles =>
      'Un dossier avec les fichiers s\'ouvre aussi : fais-les tous glisser dans la zone du message, pour qu\'ils partent en tant que fichiers.';

  @override
  String get handoffStepCopyAnswer =>
      'Quand la réponse est terminée, appuyez sur le bouton de copie en dessous.';

  @override
  String get handoffStepComeBack =>
      'Revenez dans Koalino : la réponse devient le résumé de la réunion, avec Annuler.';

  @override
  String get handoffStepAskThere =>
      'Posez ensuite vos questions sur la réunion là-bas.';

  @override
  String openSite(String site) {
    return 'Ouvrir $site';
  }

  @override
  String writtenBySourceOn(String source, String createdAt) {
    return 'Écrit par $source le $createdAt';
  }

  @override
  String get noRecordingWriteNotesOrAttach =>
      'Pas d\'enregistrement. Écrivez des notes ou joignez des fichiers : les résumés et le chat les utilisent.';

  @override
  String handoffStepOpensFilled(String site) {
    return '$site s\'ouvre avec la réunion déjà dans la zone de message.';
  }

  @override
  String handoffStepOpensCopied(String site) {
    return '$site s\'ouvre et Koalino copie la réunion.';
  }

  @override
  String get handoffStepSend => 'Appuyez sur Entrée pour l\'envoyer.';

  @override
  String get handoffStepPasteSend =>
      'Cliquez dans la zone de message, collez la réunion (Ctrl+V) et appuyez sur Entrée.';

  @override
  String get chatHandoffIntro =>
      'Pose tes questions dans ta propre app d\'IA, avec ton propre compte. Koalino lui donne les notes et les transcriptions, et les photos et fichiers les accompagnent en tant que fichiers.';
}
