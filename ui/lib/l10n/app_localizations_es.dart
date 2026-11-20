// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Spanish Castilian (`es`).
class AppLocalizationsEs extends AppLocalizations {
  AppLocalizationsEs([String locale = 'es']) : super(locale);

  @override
  String get unableToCheckTheAppLock =>
      'No se pudo comprobar el bloqueo de la app. Inténtalo de nuevo.';

  @override
  String get unableToShowTheUnlockPrompt =>
      'No se pudo mostrar la solicitud de desbloqueo. Inténtalo de nuevo.';

  @override
  String get setAScreenLockOnThis =>
      'Primero configura un bloqueo de pantalla en este dispositivo.';

  @override
  String get unableToChangeTheAppLock =>
      'No se pudo cambiar el bloqueo de la app. Inténtalo de nuevo.';

  @override
  String get aCallApp => 'Una app de llamadas';

  @override
  String get call => 'Llamada';

  @override
  String call2(Object app) {
    return 'Llamada de $app';
  }

  @override
  String theCallEndedSoTheRecording(Object tiedApp) {
    return 'La llamada de $tiedApp terminó, así que la grabación se detuvo.';
  }

  @override
  String get restartYourBrowserAndCallApps =>
      'Reinicia una vez el navegador y las apps de llamadas para que Koalino pueda leer quién habla en las llamadas.';

  @override
  String get unableToLoadDevices => 'No se pudieron cargar los dispositivos';

  @override
  String get thisDeviceIsNotOnA =>
      'Este dispositivo aún no está en una red. Conéctalo e inténtalo de nuevo.';

  @override
  String get thatDidNotWorkTryAgain => 'No funcionó. Inténtalo de nuevo.';

  @override
  String get use1To64CharactersWithout =>
      'Usa de 1 a 64 caracteres, sin saltos de línea.';

  @override
  String get theNameCouldNotBeSaved =>
      'No se pudo guardar el nombre. Inténtalo de nuevo.';

  @override
  String get thisInviteHasExpiredMakeA =>
      'Esta invitación ha caducado. Crea una nueva en el otro dispositivo.';

  @override
  String get thisInviteWasMadeOnThis =>
      'Esta invitación se creó en este dispositivo. Úsala en el otro.';

  @override
  String get thatDeviceIsAlreadyPaired => 'Ese dispositivo ya está vinculado.';

  @override
  String get thatIsNotAValidInvite =>
      'No es una invitación válida. Cópiala de nuevo.';

  @override
  String get deviceSyncIsNotRunning =>
      'La sincronización de dispositivos no está activa.';

  @override
  String get theOtherDeviceIsNoLonger =>
      'El otro dispositivo ya no se está vinculando. Inténtalo de nuevo.';

  @override
  String get theOtherAppCouldNotBe => 'No se pudo abrir la otra app.';

  @override
  String get theOtherApp => 'la otra app';

  @override
  String app(Object displayName) {
    return 'app $displayName';
  }

  @override
  String get unableToChangeTheLogLevel =>
      'No se pudo cambiar el nivel de registro';

  @override
  String logsExportedTo(Object filePath) {
    return 'Registros exportados a $filePath';
  }

  @override
  String get unableToShareTheLogs => 'No se pudieron compartir los registros';

  @override
  String get noLogsToExportYet => 'Aún no hay registros que exportar';

  @override
  String get unableToExportTheLogs => 'No se pudieron exportar los registros';

  @override
  String couldNotBeAttached(Object attached, Object failedPaths) {
    return '$attached; $failedPaths no se pudieron adjuntar';
  }

  @override
  String get theFilesCouldNotBeAttached =>
      'No se pudieron adjuntar los archivos.';

  @override
  String get unableToShareTheExport => 'No se pudo compartir la exportación';

  @override
  String exportedTo(Object filePath) {
    return 'Exportado a $filePath';
  }

  @override
  String get exportSaved => 'Exportación guardada';

  @override
  String get unableToSaveTheExport => 'No se pudo guardar la exportación';

  @override
  String get thisMeetingNoLongerExists => 'Esta reunión ya no existe';

  @override
  String get exportIsUnavailableUntilTheApp =>
      'La exportación no está disponible hasta que la app se haya iniciado';

  @override
  String get unableToExportTheMeeting => 'No se pudo exportar la reunión';

  @override
  String get unableToLoadMeetings => 'No se pudieron cargar las reuniones';

  @override
  String get unableToLoadThisMeeting => 'No se pudo cargar esta reunión';

  @override
  String get enterATitle => 'Escribe un título.';

  @override
  String titlesCanBeAtMostCharacters(
      Object maxMeetingTitleCharacters, Object runes) {
    return 'Los títulos pueden tener como máximo $maxMeetingTitleCharacters caracteres (tienes $runes).';
  }

  @override
  String get titlesCanTContainLineBreaks =>
      'Los títulos no pueden contener saltos de línea ni caracteres de control.';

  @override
  String get notPairedYetMakeTheRelay =>
      'Aún no vinculado. Haz visible el relé y luego vincúlalo en Dispositivos cercanos, en Dispositivos.';

  @override
  String get connectingToTheRelay => 'Conectando con el relé…';

  @override
  String get connectedToTheRelayAskingIt =>
      'Conectado al relé. Pidiéndole que acepte conexiones…';

  @override
  String get connectedYourOtherDevicesCanReach =>
      'Conectado. Tus otros dispositivos pueden llegar a este a través del relé.';

  @override
  String get savedThisAppliesNowWithNo =>
      'Guardado. Se aplica ya, sin reiniciar.';

  @override
  String get thatRelayAddressIsNotValid =>
      'Esa dirección de relé no es válida. Tiene esta forma: /ip4/203.0.113.5/tcp/4001/p2p/12D3Koo…';

  @override
  String get aDeviceAddressIsNotValid =>
      'Una dirección de dispositivo no es válida. Cada una termina en /p2p/ y el ID del dispositivo.';

  @override
  String get unableToSaveTheAddresses =>
      'No se pudieron guardar las direcciones.';

  @override
  String get thePhotoCouldNotBeSaved => 'No se pudo guardar la foto.';

  @override
  String get enterAName => 'Escribe un nombre.';

  @override
  String namesCanBeAtMostCharacters(Object maxMeetingTitleCharacters) {
    return 'Los nombres pueden tener como máximo $maxMeetingTitleCharacters caracteres.';
  }

  @override
  String get namesCanTContainLineBreaks =>
      'Los nombres no pueden contener saltos de línea ni caracteres de control.';

  @override
  String get unableToLoadProjects => 'No se pudieron cargar los proyectos';

  @override
  String get unableToLoadThisProject => 'No se pudo cargar este proyecto';

  @override
  String get theFilesCouldNotBeAdded => 'No se pudieron añadir los archivos.';

  @override
  String couldNotBeAdded(Object failedPaths) {
    return '$failedPaths no se pudieron añadir';
  }

  @override
  String get unableToStartRecording => 'No se pudo iniciar la grabación';

  @override
  String get unableToPauseRecording => 'No se pudo pausar la grabación';

  @override
  String get unableToResumeRecording => 'No se pudo reanudar la grabación';

  @override
  String get unableToStopRecording => 'No se pudo detener la grabación';

  @override
  String get recordingSaved => 'Grabación guardada';

  @override
  String get microphoneAccessIsNeededToRecord =>
      'Se necesita acceso al micrófono para grabar';

  @override
  String get keepTheAppOpenAndroidDid =>
      'Mantén la app abierta: Android no permitió grabar en segundo plano';

  @override
  String get unableToOpenTheMicrophone => 'No se pudo abrir el micrófono';

  @override
  String saved(Object label) {
    return '$label guardada.';
  }

  @override
  String removed(Object label) {
    return '$label eliminada.';
  }

  @override
  String isNowTheDefault(Object id) {
    return '$id es ahora la predeterminada.';
  }

  @override
  String answered(Object id, Object reply) {
    return '$id respondió: $reply';
  }

  @override
  String get promptSaved => 'Prompt guardado.';

  @override
  String get theBuiltInPromptIsBack => 'Se ha restaurado el prompt integrado.';

  @override
  String get finishedMeetingsWillBeSummarizedAutomatically =>
      'Las reuniones terminadas se resumirán automáticamente.';

  @override
  String get automaticSummariesAreOff =>
      'Los resúmenes automáticos están desactivados.';

  @override
  String get thisComputerNoLongerWritesSummaries =>
      'Este ordenador ya no escribe resúmenes ni responde chats para tus otros dispositivos.';

  @override
  String nowWritesTheSummariesAndAnswers(Object id) {
    return '$id ahora escribe los resúmenes y responde los chats que piden tus otros dispositivos.';
  }

  @override
  String get requestCancelled => 'Solicitud cancelada.';

  @override
  String get thisMeetingUsesTheSettingsPrompt =>
      'Esta reunión vuelve a usar el prompt de Ajustes.';

  @override
  String get promptSavedForThisMeeting => 'Prompt guardado para esta reunión.';

  @override
  String get summarySaved => 'Resumen guardado.';

  @override
  String get theConnection => 'La conexión';

  @override
  String get summaryWritten => 'Resumen escrito.';

  @override
  String theTranscriptWasReadInParts(Object parts) {
    return ' La transcripción se leyó en $parts partes.';
  }

  @override
  String leftOut(Object sent, Object imagesSkipped) {
    return ' $sent, $imagesSkipped omitidas.';
  }

  @override
  String get theModelStoppedBeforeTheEnd =>
      ' El modelo se detuvo antes del final.';

  @override
  String get waitingForYourComputerItWrites =>
      'Esperando a tu ordenador. Escribirá el resumen cuando esté encendido, conectado a este dispositivo y configurado para responder en Ajustes → Conexiones de IA.';

  @override
  String get yourComputerIsWritingTheSummary =>
      'Tu ordenador está escribiendo el resumen…';

  @override
  String yourComputerCouldNotWriteThe(Object message) {
    return 'Tu ordenador no pudo escribir el resumen: $message';
  }

  @override
  String get unableToRefreshTelemetry => 'No se pudo actualizar la telemetría';

  @override
  String get automatic => 'Automático';

  @override
  String get transcriptionLanguage => 'Idioma de transcripción';

  @override
  String get transcriptionLanguages => 'Idiomas de transcripción';

  @override
  String get theLanguageCouldNotBeChanged => 'No se pudo cambiar el idioma.';

  @override
  String get thisDevice => 'Este dispositivo';

  @override
  String get bestAvailable => 'El mejor disponible';

  @override
  String get aRemovedDevice => 'Un dispositivo eliminado';

  @override
  String get theTranscriptionDeviceCouldNotBe =>
      'No se pudo cambiar el dispositivo de transcripción. Comprueba que sigue vinculado.';

  @override
  String get downloadFinishedYouCanNowChoose =>
      'Descarga terminada. Ya puedes elegir este modelo.';

  @override
  String get downloadCancelled => 'Descarga cancelada.';

  @override
  String get theDownloadStoppedNoConnectionTo =>
      'La descarga se detuvo: no hay conexión con el servidor. Tu modelo actual sigue funcionando.';

  @override
  String get theDownloadedFileFailedItsCheck =>
      'El archivo descargado no superó la verificación y se eliminó. Tu modelo actual sigue funcionando.';

  @override
  String get theModelCouldNotBeSaved =>
      'No se pudo guardar el modelo. Comprueba el espacio libre. Tu modelo actual sigue funcionando.';

  @override
  String get theDownloadServerRefusedTheFile =>
      'El servidor de descarga rechazó el archivo. Tu modelo actual sigue funcionando.';

  @override
  String get savedTheNextRecordingUsesThis =>
      'Guardado. La próxima grabación usará este modelo.';

  @override
  String get savedTheNextRecordingUsesThis2 =>
      'Guardado. La próxima grabación lo usará.';

  @override
  String get savedTheNextRecordingUsesThe =>
      'Guardado. La próxima grabación usará la elección automática.';

  @override
  String get savedMeetingsRecordedFromNowOn =>
      'Guardado. Las reuniones grabadas a partir de ahora lo usarán.';

  @override
  String get savedTheNextRecordingUsesThese =>
      'Guardado. La próxima grabación usará estos nombres y términos.';

  @override
  String get eachTermCanHaveAtMost =>
      'Cada término puede tener como máximo 60 caracteres, y todos juntos como máximo 500.';

  @override
  String get chooseAWindowBetween1And => 'Elige un periodo entre 1 y 60 días.';

  @override
  String get theDownloadCouldNotStartPlease =>
      'No se pudo iniciar la descarga. Inténtalo de nuevo.';

  @override
  String get savedTheNextRecordingIsTranscribed =>
      'Guardado. La próxima grabación se transcribirá en la NPU.';

  @override
  String get savedTheNextRecordingIsTranscribed2 =>
      'Guardado. La próxima grabación se transcribirá en el procesador.';

  @override
  String get theNpuFilesAreNotAll =>
      'Aún no se han descargado todos los archivos de la NPU.';

  @override
  String get theNpuFilesWereRemoved => 'Se eliminaron los archivos de la NPU.';

  @override
  String get theNpuFilesCouldNotBe =>
      'No se pudieron eliminar los archivos de la NPU. Inténtalo de nuevo.';

  @override
  String get downloadedRecordingsNowTranscribeOnThe =>
      'Descargado. Las grabaciones ahora se transcriben en la NPU.';

  @override
  String get theDownloadStoppedNoConnectionTo2 =>
      'La descarga se detuvo: no hay conexión con el servidor.';

  @override
  String get aDownloadedFileFailedItsCheck =>
      'Un archivo descargado no superó la verificación y se eliminó.';

  @override
  String get theFilesCouldNotBeSaved =>
      'No se pudieron guardar los archivos. Comprueba el espacio libre (se necesitan unos 800 MB).';

  @override
  String get theDownloadServerRefusedTheFiles =>
      'El servidor de descarga rechazó los archivos.';

  @override
  String get theNextRecordingTriesTheGraphics =>
      'La próxima grabación volverá a probar el chip gráfico.';

  @override
  String get thatModelWonTBeProposed => 'Ese modelo no se volverá a proponer.';

  @override
  String get modelRemoved => 'Modelo eliminado.';

  @override
  String get anotherModelIsDownloadingWaitFor =>
      'Se está descargando otro modelo. Espera a que termine.';

  @override
  String get thereIsnTEnoughFreeSpace =>
      'No hay suficiente espacio libre para este modelo.';

  @override
  String get thatDidnTWorkPleaseTry => 'No funcionó. Inténtalo de nuevo.';

  @override
  String get unableToLoadVoiceGallery =>
      'No se pudo cargar la Galería de voces';

  @override
  String get unableToPlayThisVoice => 'No se pudo reproducir esta voz';

  @override
  String get unableToDeleteThisVoice => 'No se pudo eliminar esta voz';

  @override
  String get voiceDeleted => 'Voz eliminada';

  @override
  String get unableToDeleteThisPerson => 'No se pudo eliminar a esta persona';

  @override
  String deleted(Object displayName) {
    return '$displayName eliminado';
  }

  @override
  String get enterANameBeforeAssigning => 'Escribe un nombre antes de asignar';

  @override
  String assignedTo(Object identityName) {
    return 'Asignado a $identityName';
  }

  @override
  String assignedToThereWasNotEnough(Object identityName) {
    return 'Asignado a $identityName. No había suficiente habla clara para reconocer esta voz en futuras grabaciones.';
  }

  @override
  String get unableToAssignTheSpeakerCluster =>
      'No se pudo asignar el grupo de intervenciones';

  @override
  String get chooseAtLeastOneVoiceTo => 'Elige al menos una voz para nombrar';

  @override
  String thereWasNotEnoughClearSpeech(Object assigned) {
    return '$assigned. No había suficiente habla clara para reconocer esta voz en futuras grabaciones.';
  }

  @override
  String get theSettingCouldNotBeSaved => 'No se pudo guardar el ajuste.';

  @override
  String get theAppIsStillStartingTry =>
      'La app aún se está iniciando. Inténtalo de nuevo.';

  @override
  String get somethingWentWrongTryAgain =>
      'Algo salió mal. Inténtalo de nuevo.';

  @override
  String isUsingTheMicrophone(Object app) {
    return '$app está usando el micrófono';
  }

  @override
  String get recordThisCall => '¿Grabar esta llamada?';

  @override
  String theAiServerRejectedTheRequest(Object detail) {
    return 'El servidor de IA rechazó la solicitud.\nEl servidor dijo: $detail';
  }

  @override
  String claudeCodeOrCodexCouldNot(Object detail) {
    return 'Claude Code o Codex no pudo responder.\nDijo: $detail';
  }

  @override
  String get typeAQuestion => 'Escribe una pregunta.';

  @override
  String get theQuestionIsTooLong => 'La pregunta es demasiado larga.';

  @override
  String get addAnAiConnectionInSettings =>
      'Añade una conexión de IA en Ajustes o elige una predeterminada.';

  @override
  String get thatAiConnectionNoLongerExists =>
      'Esa conexión de IA ya no existe.';

  @override
  String get thereIsNothingToTalkAbout =>
      'Aún no hay nada de qué hablar: ni transcripción, ni resumen, ni notas, ni archivos.';

  @override
  String get theAiConnectionSContextIs =>
      'El contexto de la conexión de IA es demasiado pequeño. Elige una con un contexto mayor.';

  @override
  String get thatMeetingNoLongerExists => 'Esa reunión ya no existe.';

  @override
  String get thatProjectNoLongerExists => 'Ese proyecto ya no existe.';

  @override
  String get theAiServerCouldNotBe =>
      'No se pudo contactar con el servidor de IA. Comprueba la conexión.';

  @override
  String get theAiServerRefusedTheApi =>
      'El servidor de IA rechazó la clave de API.';

  @override
  String get theAiServerIsBusyTry =>
      'El servidor de IA está ocupado. Inténtalo más tarde.';

  @override
  String get theModelDeclinedToAnswer => 'El modelo se negó a responder.';

  @override
  String get claudeCodeOrCodexIsNot =>
      'Claude Code o Codex no está instalado en este ordenador, o la app no lo encuentra.';

  @override
  String get theNotesCouldNotBeRead => 'No se pudieron leer las notas.';

  @override
  String get noPairedComputerAnsweredOnA =>
      'Ningún ordenador vinculado respondió. En un ordenador, abre Ajustes → IA, elige una conexión en \"Resúmenes para mis otros dispositivos\" y mantenlo encendido y conectado.';

  @override
  String get theComputerTookTooLongTo =>
      'El ordenador tardó demasiado en responder.';

  @override
  String get theComputerDisconnectedBeforeAnsweringTry =>
      'El ordenador se desconectó antes de responder. Inténtalo de nuevo.';

  @override
  String get theComputerAnswersAtMost30 =>
      'El ordenador responde como máximo 30 preguntas por hora. Inténtalo más tarde.';

  @override
  String get theComputerIsAnsweringOtherQuestions =>
      'El ordenador está respondiendo otras preguntas. Inténtalo en un momento.';

  @override
  String get pairedDevicesAreOffOnThis =>
      'Los dispositivos vinculados están desactivados en este teléfono.';

  @override
  String get theAiServerCouldNotAnswer =>
      'El servidor de IA no pudo responder. Inténtalo de nuevo.';

  @override
  String get exportMeeting => 'Exportar reunión';

  @override
  String get openaiCompatibleServer => 'Servidor compatible con OpenAI';

  @override
  String get localServerOllama => 'Servidor local (Ollama)';

  @override
  String get claudeCodeClaudePlan => 'Claude Code (plan de Claude)';

  @override
  String get codexChatgptPlan => 'Codex (plan de ChatGPT)';

  @override
  String get checkTheAddressAndModelPlain =>
      'Comprueba la dirección y el modelo. El http:// simple solo funciona en este dispositivo o en la red local.';

  @override
  String get thatConnectionNoLongerExists => 'Esa conexión ya no existe.';

  @override
  String get addAnAiConnectionInSettings2 =>
      'Primero añade una conexión de IA en Ajustes.';

  @override
  String get thisMeetingHasNoTranscriptOr =>
      'Esta reunión aún no tiene transcripción ni notas.';

  @override
  String get thisMeetingIsTooLongFor =>
      'Esta reunión es demasiado larga para esta conexión. Elige una con un contexto mayor.';

  @override
  String get theAiServerCouldNotBe2 =>
      'No se pudo contactar con el servidor de IA.';

  @override
  String get theAiServerRefusedAccessUse =>
      'El servidor de IA rechazó el acceso. Revisa la clave de API de la conexión.';

  @override
  String get theAiServerIsBusyOr =>
      'El servidor de IA está ocupado o superó su límite. Inténtalo más tarde.';

  @override
  String get theAiServerRejectedTheRequest2 =>
      'El servidor de IA rechazó la solicitud. Comprueba el nombre del modelo.';

  @override
  String get theAiServerFailedTryAgain =>
      'El servidor de IA falló. Inténtalo de nuevo.';

  @override
  String get theAiServerSAnswerCould =>
      'No se pudo leer la respuesta del servidor de IA.';

  @override
  String get claudeCodeOrCodexCouldNot2 =>
      'Claude Code o Codex no pudo responder. Comprueba que tiene la sesión iniciada.';

  @override
  String get theSettingsCouldNotBeSaved =>
      'No se pudieron guardar los ajustes.';

  @override
  String get theMeetingCouldNotBeRead =>
      'No se pudo leer ni guardar la reunión.';

  @override
  String get error => 'Error';

  @override
  String get warning => 'Advertencia';

  @override
  String get infoDefault => 'Info (predeterminado)';

  @override
  String get debug => 'Depuración';

  @override
  String get trace => 'Traza';

  @override
  String get thisDevice2 => 'este dispositivo';

  @override
  String get anotherDevice => 'otro dispositivo';

  @override
  String notConnected(Object label) {
    return '$label (no conectado)';
  }

  @override
  String itIsKeptEncryptedUntil(Object until) {
    return ' Se guarda cifrado hasta el $until.';
  }

  @override
  String get recordedOnThisDeviceOnly => 'Grabada solo en este dispositivo.';

  @override
  String recordedOnThisDeviceOnlyA(Object until) {
    return 'Grabada solo en este dispositivo. Una grabación de ella hecha en otro dispositivo aún puede combinarse hasta el $until.';
  }

  @override
  String combinesTheRecordingsNothingIsLeft(Object combiner) {
    return '$combiner combina las grabaciones; no queda nada que enviar desde aquí.';
  }

  @override
  String waitingForAudioFrom(Object names, Object kept) {
    return 'Esperando el audio de $names.$kept';
  }

  @override
  String combinesTheRecordingsWaitingForAudio(Object combiner, Object names) {
    return '$combiner combina las grabaciones; esperando el audio de $names.';
  }

  @override
  String get audioFromEveryDeviceIsHere =>
      'Ya está el audio de todos los dispositivos; la transcripción se combina cuando este dispositivo esté libre.';

  @override
  String thisDeviceSAudioStillHas(Object combiner, Object kept) {
    return 'El audio de este dispositivo aún tiene que llegar a $combiner.$kept';
  }

  @override
  String get transcriptCombinedFromEveryDevice =>
      'Transcripción combinada de todos los dispositivos.';

  @override
  String notCombinedAudioFromDidNot(Object plainNames) {
    return 'No combinada: el audio de $plainNames no llegó en tres semanas.';
  }

  @override
  String meeting(Object createdAt) {
    return 'Reunión $createdAt';
  }

  @override
  String get writeSomethingFirst => 'Escribe algo primero.';

  @override
  String get theTextIsTooLong => 'El texto es demasiado largo.';

  @override
  String get thatPartOfTheTranscriptChanged =>
      'Esa parte de la transcripción cambió mientras tanto. Vuelve a abrir la reunión.';

  @override
  String get theMeetingCouldNotBeSaved => 'No se pudo guardar la reunión.';

  @override
  String get chooseAFullFolderPathInside =>
      'Elige la ruta completa de una carpeta, dentro de tu carpeta personal pero no la carpeta personal en sí.';

  @override
  String get theFolderCouldNotBeWritten =>
      'No se pudo escribir en la carpeta. Comprueba que existe y que puedes escribir en ella.';

  @override
  String get theNotesCouldNotBeRead2 =>
      'No se pudieron leer ni guardar las notas.';

  @override
  String get theInviteAnswerIsIncomplete =>
      'La respuesta a la invitación está incompleta.';

  @override
  String get theRenameAnswerIsIncomplete =>
      'La respuesta al cambio de nombre está incompleta.';

  @override
  String get thisDeviceHasNoCamera => 'Este dispositivo no tiene cámara.';

  @override
  String get theCameraIsNotOpen => 'La cámara no está abierta.';

  @override
  String get title => 'Título';

  @override
  String get summary => 'Resumen';

  @override
  String get notes => 'Notas';

  @override
  String get transcript => 'Transcripción';

  @override
  String get projectName => 'Nombre del proyecto';

  @override
  String file(Object fileName) {
    return 'Archivo $fileName';
  }

  @override
  String get typeWhatToLookFor => 'Escribe qué buscar.';

  @override
  String get typeAtLeastTwoLetters => 'Escribe al menos dos letras.';

  @override
  String get theSearchIsTooLong => 'La búsqueda es demasiado larga.';

  @override
  String get exportLogs => 'Exportar registros';

  @override
  String about(Object appName) {
    return 'Acerca de $appName';
  }

  @override
  String get versionLicenseAndTheSoftwareIt =>
      'Versión, licencia y el software que usa.';

  @override
  String remove(Object label) {
    return '¿Eliminar $label?';
  }

  @override
  String get itsApiKeyIsDeletedFrom =>
      'Su clave de API se borra de este dispositivo. Los resúmenes ya escritos se conservan.';

  @override
  String get cancel => 'Cancelar';

  @override
  String get remove2 => 'Eliminar';

  @override
  String get aiConnections => 'Conexiones de IA';

  @override
  String get addConnection => 'Añadir conexión';

  @override
  String get summariesSendTheMeetingSTranscript =>
      'Los resúmenes envían la transcripción, las notas y las fotos de la reunión al servidor de la conexión. Un servidor en tu red local los mantiene allí.';

  @override
  String get noConnectionsYet => 'Aún no hay conexiones';

  @override
  String get addAServerOnYourNetwork =>
      'Añade un servidor de tu red (Ollama, LM Studio, llama.cpp), o Claude Code o Codex en este ordenador.';

  @override
  String get summarizeMeetingsAutomatically =>
      'Resumir reuniones automáticamente';

  @override
  String get withTheDefaultConnectionWhenA =>
      'Con la conexión predeterminada, cuando la transcripción de una reunión es definitiva.';

  @override
  String get summariesForMyOtherDevices =>
      'Resúmenes para mis otros dispositivos';

  @override
  String get aPairedPhoneCanAskThis =>
      'Un teléfono vinculado puede pedir a este ordenador que escriba un resumen o responda un chat con esta conexión, a partir de la copia de tus notas en este ordenador. Cuenta para ese plan; como máximo 10 resúmenes y 30 respuestas de chat por hora.';

  @override
  String get off => 'Desactivado';

  @override
  String get summaryPrompt => 'Prompt del resumen';

  @override
  String placeholdersTheTranscriptIsAlwaysSent(Object placeholders) {
    return 'Marcadores: $placeholders. La transcripción siempre se envía después del prompt.';
  }

  @override
  String get savePrompt => 'Guardar prompt';

  @override
  String get resetToBuiltIn => 'Restablecer el integrado';

  @override
  String get defaultLabel => 'Predeterminada';

  @override
  String get installed => 'Instalado';

  @override
  String get notFoundOnThisComputer => 'No encontrado en este ordenador';

  @override
  String get unencryptedHttpLocalNetworkOnly =>
      'http sin cifrar, solo red local';

  @override
  String get runsWithYourPlanSSign =>
      'Funciona con la sesión de tu plan en este ordenador';

  @override
  String actionsFor(Object label) {
    return 'Acciones para $label';
  }

  @override
  String get makeDefault => 'Hacer predeterminada';

  @override
  String get test => 'Probar';

  @override
  String get edit => 'Editar';

  @override
  String get enterANameAnAddressAnd =>
      'Escribe un nombre, una dirección y un modelo.';

  @override
  String get theContextIsANumberOf =>
      'El contexto es un número de tokens, como 8192.';

  @override
  String edit2(Object label) {
    return 'Editar $label';
  }

  @override
  String get name => 'Nombre';

  @override
  String get protocol => 'Protocolo';

  @override
  String get runsTheToolInstalledAndSigned =>
      'Usa la herramienta instalada y con sesión iniciada en este ordenador, así tu plan de Claude o ChatGPT escribe los resúmenes en lugar del crédito de API. La reunión va a Anthropic u OpenAI con esa cuenta.';

  @override
  String get address => 'Dirección';

  @override
  String get httpsAlwaysHttpOnlyOnThis =>
      'https:// siempre; http:// solo en este dispositivo o en la red local.';

  @override
  String get model => 'Modelo';

  @override
  String get leaveEmptyForTheToolS =>
      'Déjalo vacío para usar el predeterminado de la herramienta.';

  @override
  String get contextTokens => 'Contexto (tokens)';

  @override
  String get saveConnection => 'Guardar conexión';

  @override
  String get koalinoIsLocked => 'Koalino está bloqueado';

  @override
  String get unlockWithYourFingerprintFaceOr =>
      'Desbloquea con tu huella, tu cara o el bloqueo de pantalla.';

  @override
  String get unlock => 'Desbloquear';

  @override
  String copied(Object what) {
    return '$what copiado';
  }

  @override
  String get aiAssistantAccess => 'Acceso del asistente de IA';

  @override
  String get letYourOwnAiAssistantSuch =>
      'Deja que tu propio asistente de IA, como Claude Code, Codex o Claude Desktop, lea tus reuniones y proyectos: títulos, resúmenes, notas, transcripciones y archivos de proyectos. Solo puede leer; no puede cambiar ni borrar nada.';

  @override
  String get whatTheAssistantReadsMayBe =>
      'Lo que lee el asistente puede enviarse a su modelo en línea, con tu cuenta en ese servicio. Actívalo solo para un asistente al que confiarías tus reuniones.';

  @override
  String get theAssistantServerIsNotInstalled =>
      'El servidor del asistente no está instalado con esta copia de la app.';

  @override
  String get letMyAiAssistantReadMy =>
      'Dejar que mi asistente de IA lea mis notas';

  @override
  String get connectYourAssistant => 'Conecta tu asistente';

  @override
  String get yourNotesAreUnlockedWithA =>
      'Tus notas se desbloquean con una frase de contraseña, que nunca se escribe en los ajustes de un asistente. El servidor solo las lee si NOTE_TAKER_ENCRYPTION_PASSPHRASE está definida donde el asistente lo inicia.';

  @override
  String get claudeCodeRunInATerminal => 'Claude Code: ejecuta en un terminal';

  @override
  String get command => 'Comando';

  @override
  String get claudeDesktopAndOtherMcpClients =>
      'Claude Desktop y otros clientes MCP: añádelo a sus ajustes';

  @override
  String get settings => 'Ajustes';

  @override
  String copy(Object title) {
    return 'Copiar: $title';
  }

  @override
  String isUsingTheMicrophoneRecordThis(Object app) {
    return '$app está usando el micrófono. ¿Grabar esta llamada?';
  }

  @override
  String get notNow => 'Ahora no';

  @override
  String get record => 'Grabar';

  @override
  String get offerToRecordCalls => 'Ofrecer grabar las llamadas';

  @override
  String get whenTeamsZoomMeetInA =>
      'Cuando Teams, Zoom, Meet en un navegador u otra app de llamadas empieza a usar el micrófono. No se graba nada hasta que tocas Grabar.';

  @override
  String get stopWhenTheCallEnds => 'Detener cuando termine la llamada';

  @override
  String get onlyForRecordingsStartedFromThe =>
      'Solo para grabaciones iniciadas desde la oferta, cuando la app de llamadas lleva 20 segundos sin usar el micrófono.';

  @override
  String get readSpeakerNamesFromTheCall =>
      'Leer los nombres de quien habla en la ventana de la llamada';

  @override
  String get whileRecordingKoalinoReadsWhoTeams =>
      'Mientras graba, Koalino lee a quién muestra Teams, Zoom o Meet hablando, para sugerir nombres para las voces desconocidas. Nada sale de este ordenador y no se nombra a nadie hasta que tocas Asignar.';

  @override
  String get deleteThisChat => '¿Eliminar este chat?';

  @override
  String get itsQuestionsAndAnswersAreDeleted =>
      'Sus preguntas y respuestas se borran de este dispositivo.';

  @override
  String get delete => 'Eliminar';

  @override
  String get thatFileIsNoLongerIn => 'Ese archivo ya no está en el proyecto.';

  @override
  String get whoAnswers => 'Quién responde';

  @override
  String get myPairedComputer => 'Mi ordenador vinculado';

  @override
  String get thisPhoneSAiConnection => 'La conexión de IA de este teléfono';

  @override
  String get deleteThisChat2 => 'Eliminar este chat';

  @override
  String get askAboutYourNotesAnswersCome =>
      'Pregunta sobre tus notas. Responde tu ordenador vinculado, que lee su propia copia de tus notas con la conexión elegida allí para tus otros dispositivos.';

  @override
  String get askAboutYourNotesTheAnswer =>
      'Pregunta sobre tus notas. Responde tu conexión de IA predeterminada, a la que se envían las notas que caben.';

  @override
  String get yourQuestion => 'Tu pregunta';

  @override
  String get ask => 'Preguntar';

  @override
  String get open => 'Abrir';

  @override
  String given(Object included) {
    return 'Incluido: $included';
  }

  @override
  String leftOutForLength(Object omitted) {
    return 'Omitido por longitud: $omitted';
  }

  @override
  String answeredBy(Object connection) {
    return 'Respondido por $connection';
  }

  @override
  String get unableToOpenTheKeychain => 'No se pudo abrir el llavero';

  @override
  String get koalinoKeepsTheKeyThatEncrypts =>
      'Koalino guarda en el llavero del sistema la clave que cifra tus perfiles de voz y la identidad del dispositivo. Desbloquea el llavero, o permite que Koalino lo use, e inténtalo de nuevo. Si este dispositivo no tiene un servicio de llavero, instala o inicia uno y reinicia Koalino.';

  @override
  String get unableToOpenYourNotes => 'No se pudieron abrir tus notas';

  @override
  String get koalinoCouldNotOpenItsDatabase =>
      'Koalino no pudo abrir su base de datos. Comprueba que hay espacio libre e inténtalo de nuevo.';

  @override
  String get unlockingYourNotes => 'Desbloqueando tus notas…';

  @override
  String get quit => 'Salir';

  @override
  String get retry => 'Reintentar';

  @override
  String delete2(Object title) {
    return '¿Eliminar $title?';
  }

  @override
  String get itsTranscriptAttachmentsAndUnknownVoices =>
      'Su transcripción, adjuntos y voces desconocidas se borran para siempre, aquí y en tus dispositivos vinculados. Las personas guardadas se conservan.';

  @override
  String get unableToDeleteThisMeetingIf =>
      'No se pudo eliminar esta reunión. Si se está grabando, detén primero la grabación.';

  @override
  String get devices => 'Dispositivos';

  @override
  String get deviceSyncIsOff =>
      'La sincronización de dispositivos está desactivada';

  @override
  String get meetingsStayOnThisDeviceUntil =>
      'Las reuniones se quedan en este dispositivo hasta que se active la sincronización.';

  @override
  String thisDevice3(Object capabilities) {
    return 'Este dispositivo\n$capabilities';
  }

  @override
  String get renameThisDevice => 'Cambiar el nombre de este dispositivo';

  @override
  String get discoverableYourOtherDevicesCanAsk =>
      'Visible: tus otros dispositivos pueden pedir vincularse durante unos minutos.';

  @override
  String get makeDiscoverable => 'Hacer visible';

  @override
  String get pairingRequests => 'Solicitudes de vinculación';

  @override
  String get acceptOnlyIfTheOtherDevice =>
      'Acepta solo si el otro dispositivo muestra el mismo código.';

  @override
  String get reject => 'Rechazar';

  @override
  String get accept => 'Aceptar';

  @override
  String get pairingInProgress => 'Vinculación en curso';

  @override
  String get waitingForTheOtherDevice => 'Esperando al otro dispositivo…';

  @override
  String get checkThatTheOtherDeviceShows =>
      'Comprueba que el otro dispositivo muestra el mismo código.';

  @override
  String get cancelPairing => 'Cancelar vinculación';

  @override
  String get codesMatch => 'Los códigos coinciden';

  @override
  String get pairedDevices => 'Dispositivos vinculados';

  @override
  String get noPairedDevicesYet => 'Aún no hay dispositivos vinculados.';

  @override
  String paired(Object pairedAt, Object hasRelay) {
    return 'Vinculado el $pairedAt\n$hasRelay';
  }

  @override
  String remove3(Object deviceName) {
    return 'Eliminar $deviceName';
  }

  @override
  String get nearbyDevices => 'Dispositivos cercanos';

  @override
  String get pair => 'Vincular';

  @override
  String get anotherNetwork => 'Otra red';

  @override
  String get showAnInviteHereAndUse =>
      'Muestra una invitación aquí y úsala en el otro dispositivo. Aun así compararás un código antes de vincularlos.';

  @override
  String get showInvite => 'Mostrar invitación';

  @override
  String get useInvite => 'Usar invitación';

  @override
  String stopsSyncingWithThisDevicePair(Object deviceName) {
    return '$deviceName deja de sincronizarse con este dispositivo. Vincúlalo de nuevo para reanudar.';
  }

  @override
  String pairingCode(Object split) {
    return 'Código de vinculación $split';
  }

  @override
  String get noOtherDevicesFoundOnThis =>
      'No se encontraron otros dispositivos en esta red. Cercanos solo encuentra dispositivos en la misma Wi-Fi. Para un dispositivo en otra red, usa una invitación.';

  @override
  String get noOtherDevicesFoundOnThis2 =>
      'No se encontraron otros dispositivos en esta red. Cercanos solo encuentra dispositivos en la misma Wi-Fi. Para un dispositivo en otra red, usa una invitación o configura un relé en Ajustes.';

  @override
  String get notConnectedItReconnectsWhenIt =>
      'No conectado. Se reconecta cuando está en línea y puede llegar a tu relé.';

  @override
  String get notConnectedPutBothDevicesOn =>
      'No conectado. Pon ambos dispositivos en la misma Wi-Fi, o configura un relé en Ajustes.';

  @override
  String get whisperOnDevice => 'Whisper (en el dispositivo)';

  @override
  String get openaiCloud => 'OpenAI (nube)';

  @override
  String gbGpu(Object round) {
    return 'GPU de $round GB';
  }

  @override
  String mbGpu(Object gpuMegabytes) {
    return 'GPU de $gpuMegabytes MB';
  }

  @override
  String get noTranscriptionModels => 'Sin modelos de transcripción';

  @override
  String get useYourOwnAiApp => 'Usar tu propia app de IA';

  @override
  String get theMeetingItsNotesAndIts =>
      'La reunión, sus notas y sus archivos van a la app que elijas, con tu propia cuenta. Para un resumen, copia allí la respuesta y vuelve, o compártela con Koalino: Koalino la guarda.';

  @override
  String summarizeIn(Object displayName) {
    return 'Resumir en $displayName';
  }

  @override
  String chatIn(Object displayName) {
    return 'Chatear en $displayName';
  }

  @override
  String get summarizeInAnotherApp => 'Resumir en otra app…';

  @override
  String get chatInAnotherApp => 'Chatear en otra app…';

  @override
  String summaryFromSavedTo(Object appName, Object title) {
    return 'Resumen de $appName guardado en \"$title\".';
  }

  @override
  String get undo => 'Deshacer';

  @override
  String get theEarlierSummaryIsBack => 'Se ha restaurado el resumen anterior.';

  @override
  String toKeepTheSummaryCopyS(Object appName) {
    return 'Para conservar el resumen, copia la respuesta de $appName y vuelve a Koalino, o comparte la respuesta con Koalino.';
  }

  @override
  String get meetings => 'Reuniones';

  @override
  String get projects => 'Proyectos';

  @override
  String get voices => 'Voces';

  @override
  String get meeting2 => 'Reunión';

  @override
  String get chatAboutThisMeeting => 'Chatear sobre esta reunión';

  @override
  String chat(Object meeting) {
    return 'Chat: $meeting';
  }

  @override
  String get thisMeeting => 'esta reunión';

  @override
  String get takePhotos => 'Hacer fotos';

  @override
  String get attachFiles => 'Adjuntar archivos';

  @override
  String get export => 'Exportar';

  @override
  String shareAs(Object label) {
    return 'Compartir como $label';
  }

  @override
  String get nameSpeakers => 'Nombrar a los hablantes';

  @override
  String get selectAMeetingToSeeIts =>
      'Selecciona una reunión para ver su transcripción.';

  @override
  String get renameMeeting => 'Cambiar nombre de la reunión';

  @override
  String get deleteMeeting => 'Eliminar reunión';

  @override
  String recordedOn(Object devices) {
    return 'Grabada en $devices';
  }

  @override
  String get split => 'Separar';

  @override
  String get longPressOrRightClickA =>
      'Mantén pulsado o haz clic derecho en un pasaje para corregirlo.';

  @override
  String get noTranscriptIsAvailable => 'No hay transcripción disponible.';

  @override
  String get theSpeakerIsAGuess => 'El hablante es una suposición';

  @override
  String get peopleTalkedAtOnceHereWords =>
      'Aquí hablaron varias personas a la vez; pueden faltar palabras';

  @override
  String get overlappingSpeech => 'Habla superpuesta';

  @override
  String get someWordsWereUnclear => 'Algunas palabras no eran claras';

  @override
  String get addedAfterTheMeeting => 'Añadido después de la reunión';

  @override
  String photoOpenFullScreen(Object fileName) {
    return 'Foto $fileName, abrir a pantalla completa';
  }

  @override
  String get save => 'Guardar';

  @override
  String get preview => 'Vista previa';

  @override
  String get editNotes => 'Editar notas';

  @override
  String get writeNotes => 'Escribir notas';

  @override
  String get loading => 'Cargando…';

  @override
  String get noNotesYet => 'Aún no hay notas.';

  @override
  String textAt(Object startTimeMs) {
    return 'Texto en $startTimeMs';
  }

  @override
  String get removedThisTranscriptWonTBe =>
      'Eliminado. Esta transcripción no se volverá a transcribir después de la reunión.';

  @override
  String get savedThisTranscriptWonTBe =>
      'Guardado. Esta transcripción no se volverá a transcribir después de la reunión.';

  @override
  String get editText => 'Editar texto';

  @override
  String get editTheTranscript => 'Editar la transcripción';

  @override
  String get leaveItEmptyToRemoveThis =>
      'Déjalo vacío para eliminar esta parte.';

  @override
  String get searchMeetings => 'Buscar en reuniones';

  @override
  String get chatWithAllYourNotes => 'Chatear con todas tus notas';

  @override
  String get chatWithAllNotes => 'Chatear con todas las notas';

  @override
  String get refresh => 'Actualizar';

  @override
  String get noMeetingsYet => 'Aún no hay reuniones';

  @override
  String get recordingsYouStartAppearHere =>
      'Las grabaciones que inicies aparecen aquí.';

  @override
  String get rename => 'Cambiar nombre';

  @override
  String get addToProject => 'Añadir a un proyecto…';

  @override
  String get summary2 => 'Resumen…';

  @override
  String get addToProject2 => 'Añadir a un proyecto';

  @override
  String writingTheSummaryOf(Object title) {
    return 'Escribiendo el resumen de $title';
  }

  @override
  String summaryOf(Object title) {
    return 'Resumen de $title';
  }

  @override
  String summarizeWith(Object label) {
    return 'Resumir con $label';
  }

  @override
  String defaultLabel2(Object model) {
    return '$model · predeterminada';
  }

  @override
  String get summarizeOnMyComputer => 'Resumir en mi ordenador';

  @override
  String get withClaudeCodeCodexOrThe =>
      'Con Claude Code, Codex o la conexión configurada allí';

  @override
  String get useYourOwnAiApp2 => 'Usar tu propia app de IA…';

  @override
  String get claudeChatgptOrAnotherAppWith =>
      'Claude, ChatGPT u otra app, con tu cuenta';

  @override
  String get openSummary => 'Abrir resumen';

  @override
  String get editPromptForThisMeeting => 'Editar el prompt de esta reunión…';

  @override
  String get useTheSettingsPrompt => 'Usar el prompt de Ajustes';

  @override
  String get promptForThisMeeting => 'Prompt de esta reunión';

  @override
  String placeholders(Object placeholders) {
    return 'Marcadores: $placeholders';
  }

  @override
  String get addAnAiConnectionInSettings3 =>
      'Añade una conexión de IA en Ajustes para escribir resúmenes.';

  @override
  String get summarizeWith2 => 'Resumir con';

  @override
  String get summarize => 'Resumir';

  @override
  String get summarizeAgain => 'Resumir de nuevo';

  @override
  String get editPromptForThisMeeting2 => 'Editar el prompt de esta reunión';

  @override
  String get writingTheSummaryALocalModel =>
      'Escribiendo el resumen… Un modelo local puede tardar unos minutos.';

  @override
  String get noSummaryYet => 'Aún no hay resumen.';

  @override
  String get writeASummary => 'Escribir un resumen';

  @override
  String writtenByYouOn(Object createdAt) {
    return 'Escrito por ti el $createdAt';
  }

  @override
  String writtenByOn(
      Object model, Object connectionLabel, Object createdAt, Object edited) {
    return 'Escrito por $model ($connectionLabel) el $createdAt$edited';
  }

  @override
  String get editSummary => 'Editar resumen';

  @override
  String get tryAgainOnMyComputer => 'Reintentar en mi ordenador';

  @override
  String get cancelRequest => 'Cancelar solicitud';

  @override
  String get notesFolder => 'Carpeta de notas';

  @override
  String get keepYourProjectsAndMeetingsAs =>
      'Guarda tus proyectos y reuniones como archivos normales en una carpeta, para leerlos y editarlos con cualquier editor. Los cambios en notes.md, summary.md y los archivos de proyecto vuelven a la app en medio minuto, y los archivos que dejes en la carpeta de un proyecto o en la de adjuntos de una reunión se añaden. La app reescribe transcript.md: corrige la transcripción en la app.';

  @override
  String get theFolderIsADecryptedCopy =>
      'La carpeta es una copia descifrada de tus notas. Quien pueda leerla puede leerlas, y las herramientas de copia de seguridad o sincronización la copiarán. Guárdala en un disco cifrado, dentro de tu propia carpeta de usuario.';

  @override
  String get folder => 'Carpeta';

  @override
  String get chooseAFolder => 'Elegir una carpeta';

  @override
  String get keepTheNotesFolder => 'Mantener la carpeta de notas';

  @override
  String get syncNow => 'Sincronizar ahora';

  @override
  String lastSyncAtFailed(Object at, Object code) {
    return 'La última sincronización a las $at falló: $code';
  }

  @override
  String lastSyncAtFilesWrittenEdits(
      Object at, Object written, Object imported) {
    return 'Última sincronización a las $at: $written archivos escritos, $imported cambios y archivos incorporados.';
  }

  @override
  String get inviteADevice => 'Invitar a un dispositivo';

  @override
  String get pairingInviteQrCode => 'Código QR de la invitación de vinculación';

  @override
  String onTheOtherDeviceOpenDevices(Object hour, Object minute) {
    return 'En el otro dispositivo, abre Dispositivos, elige Usar invitación y pega este enlace. Puedes escanear el código con una app de cámara para copiarlo. Funciona hasta las $hour:$minute.';
  }

  @override
  String get close => 'Cerrar';

  @override
  String get inviteLinkCopied => 'Enlace de invitación copiado';

  @override
  String get copyLink => 'Copiar enlace';

  @override
  String get deviceFoundChoosePairUnderNearby =>
      'Dispositivo encontrado. Elige Vincular en Dispositivos cercanos.';

  @override
  String get useAnInvite => 'Usar una invitación';

  @override
  String get inviteLink => 'Enlace de invitación';

  @override
  String get findDevice => 'Buscar dispositivo';

  @override
  String get pairWithAnotherDevice => '¿Vincular con otro dispositivo?';

  @override
  String get youOpenedAPairingInviteKoalino =>
      'Abriste una invitación de vinculación. Koalino buscará el dispositivo que la muestra, y ambos mostrarán un código para comparar antes de compartir nada.';

  @override
  String get pairingStartedCheckThatBothDevices =>
      'Vinculación iniciada. Comprueba que ambos dispositivos muestran el mismo código.';

  @override
  String get done => 'Listo';

  @override
  String get photosAreAddedToTheMeeting =>
      'Las fotos se añaden a la reunión a medida que las haces.';

  @override
  String get takePhoto => 'Hacer foto';

  @override
  String get theCameraIsUnavailable => 'La cámara no está disponible.';

  @override
  String search(Object name) {
    return 'Buscar en $name';
  }

  @override
  String get noteSaved => 'Nota guardada';

  @override
  String get theNoteCouldNotBeSaved => 'No se pudo guardar la nota.';

  @override
  String couldNotBeAdded2(Object failed) {
    return '$failed no se pudieron añadir.';
  }

  @override
  String get renameProject => 'Cambiar nombre del proyecto';

  @override
  String get theProjectCouldNotBeRenamed =>
      'No se pudo cambiar el nombre del proyecto.';

  @override
  String delete3(Object project) {
    return '¿Eliminar \"$project\"?';
  }

  @override
  String get itsFilesNotesAndEMails =>
      'Sus archivos, notas y correos se borran en todos los dispositivos vinculados. Sus reuniones se conservan, fuera de cualquier proyecto.';

  @override
  String get theProjectCouldNotBeDeleted => 'No se pudo eliminar el proyecto.';

  @override
  String get itIsDeletedOnEveryPaired =>
      'Se borra en todos los dispositivos vinculados.';

  @override
  String get theFileCouldNotBeDeleted => 'No se pudo eliminar el archivo.';

  @override
  String get theMeetingCouldNotBeRemoved =>
      'No se pudo quitar la reunión del proyecto.';

  @override
  String get project => 'Proyecto';

  @override
  String get searchThisProject => 'Buscar en este proyecto';

  @override
  String get chatAboutThisProject => 'Chatear sobre este proyecto';

  @override
  String get addFiles => 'Añadir archivos';

  @override
  String get writeANote => 'Escribir una nota';

  @override
  String get addMeetings => 'Añadir reuniones';

  @override
  String get deleteProject => 'Eliminar proyecto';

  @override
  String get addThemHereOrFromA =>
      'Añádelas aquí o desde la página de una reunión.';

  @override
  String get removeFromProject => 'Quitar del proyecto';

  @override
  String get filesNotesAndEMails => 'Archivos, notas y correos';

  @override
  String get nothingAddedYet => 'Aún no se ha añadido nada';

  @override
  String get addPicturesMarkdownPdfsEMails =>
      'Añade imágenes, Markdown, PDF, correos (.eml) o documentos.';

  @override
  String get onItsWayFromAPaired => 'En camino desde un dispositivo vinculado';

  @override
  String get newNote => 'Nueva nota';

  @override
  String get noteMarkdown => 'Nota (Markdown)';

  @override
  String get everyMeetingIsAlreadyInA =>
      'Todas las reuniones ya están en un proyecto.';

  @override
  String get add => 'Añadir';

  @override
  String get moveToProject => 'Mover al proyecto';

  @override
  String get noProject => 'Sin proyecto';

  @override
  String get newProject => 'Nuevo proyecto…';

  @override
  String get newProject2 => 'Nuevo proyecto';

  @override
  String get create => 'Crear';

  @override
  String get theProjectCouldNotBeCreated => 'No se pudo crear el proyecto.';

  @override
  String get theMeetingCouldNotBeMoved => 'No se pudo mover la reunión.';

  @override
  String get removedFromItsProject => 'Quitada de su proyecto';

  @override
  String movedTo(Object project) {
    return 'Movida a $project';
  }

  @override
  String get searchProjects => 'Buscar en proyectos';

  @override
  String get noProjectsYetAProjectGathers =>
      'Aún no hay proyectos. Un proyecto reúne reuniones con los archivos, notas y correos que les corresponden.';

  @override
  String get theRecordingIsStillStartingTry =>
      'La grabación aún se está iniciando; inténtalo de nuevo.';

  @override
  String recording(Object title) {
    return 'Grabando \"$title\"';
  }

  @override
  String get recording2 => 'Grabando';

  @override
  String paused(Object title) {
    return '\"$title\" en pausa';
  }

  @override
  String get paused2 => 'En pausa';

  @override
  String get finishingTranscription => 'Terminando la transcripción…';

  @override
  String get readyToRecord => 'Listo para grabar';

  @override
  String get meetingTitle => 'Título de la reunión';

  @override
  String get enterMeetingTitleOptional =>
      'Escribe el título de la reunión (opcional)';

  @override
  String get leaveEmptyToNameTheMeeting =>
      'Déjalo vacío para nombrar la reunión por su hora de inicio';

  @override
  String get startingRecording => 'Iniciando la grabación';

  @override
  String get startRecording => 'Iniciar grabación';

  @override
  String get pause => 'Pausar';

  @override
  String get resume => 'Reanudar';

  @override
  String get stop => 'Detener';

  @override
  String get language => 'Idioma';

  @override
  String get aChangeAppliesToWhatIs =>
      'Un cambio se aplica a lo que se diga después';

  @override
  String get liveTranscript => 'Transcripción en directo';

  @override
  String get theTranscriptAppearsHereAFew =>
      'La transcripción aparece aquí unos segundos después de cada pausa al hablar.';

  @override
  String get nearbyDevicesSeeThisNameWhile =>
      'Los dispositivos cercanos ven este nombre mientras este dispositivo está visible. Los dispositivos ya vinculados conservan el nombre que conocieron.';

  @override
  String get meetingRenamed => 'Reunión renombrada';

  @override
  String get thisTitleIsnTAllowed => 'Este título no está permitido.';

  @override
  String get couldnTSaveTheNewTitle =>
      'No se pudo guardar el nuevo título. Inténtalo de nuevo.';

  @override
  String get theNoteTakingEngineIsnT =>
      'El motor de notas no está disponible en este momento.';

  @override
  String get thisMeetingNoLongerExists2 => 'Esta reunión ya no existe.';

  @override
  String get couldnTRenameTheMeeting =>
      'No se pudo cambiar el nombre de la reunión.';

  @override
  String get thisFileIsStillOnIts =>
      'Este archivo aún está en camino desde un dispositivo vinculado.';

  @override
  String get thisPictureCannotBeShown => 'No se puede mostrar esta imagen.';

  @override
  String get thisKindOfDocumentCanT =>
      'Este tipo de documento aún no se puede mostrar en la app.';

  @override
  String get thisFileCannotBeOpened => 'No se puede abrir este archivo.';

  @override
  String get thisFileCannotBeRead => 'No se puede leer este archivo.';

  @override
  String get thisEMailCannotBeRead => 'No se puede leer este correo.';

  @override
  String get noSubject => '(sin asunto)';

  @override
  String get from => 'De';

  @override
  String get to => 'Para';

  @override
  String get cc => 'CC';

  @override
  String get sent => 'Enviado';

  @override
  String attachments(Object attachmentNames) {
    return 'Adjuntos: $attachmentNames';
  }

  @override
  String get theRestOfThisEMail =>
      'El resto de este correo es demasiado largo para mostrarlo.';

  @override
  String get searching => 'Buscando…';

  @override
  String get nothingFound => 'No se encontró nada.';

  @override
  String get search2 => 'Buscar';

  @override
  String get pairYourDevicesAndSeeWhich =>
      'Vincula tus dispositivos y mira cuáles están conectados.';

  @override
  String get transcriptionModel => 'Modelo de transcripción';

  @override
  String get chooseTheWhisperModel => 'Elige el modelo de Whisper.';

  @override
  String get languageModelsForMeetingSummariesAnd =>
      'Modelos de lenguaje para resumir reuniones, y el prompt del resumen.';

  @override
  String get keepProjectsAndMeetingsAsFiles =>
      'Guarda proyectos y reuniones como archivos que puedes editar con cualquier editor.';

  @override
  String get letYourOwnAssistantSuchAs =>
      'Deja que tu propio asistente, como Claude Code, lea tus notas.';

  @override
  String get appLock => 'Bloqueo de la app';

  @override
  String get defaultLanguage => 'Idioma predeterminado';

  @override
  String get usedForNewRecordingsAutoDetects =>
      'Se usa para las nuevas grabaciones. Auto detecta el idioma mientras la gente habla.';

  @override
  String get transcribeOn => 'Transcribir en';

  @override
  String get anotherDeviceReceivesTheRecordingS =>
      'Otro dispositivo recibe el audio de la grabación, cifrado, y lo borra una vez transcrito. Si no está disponible, transcribe este dispositivo.';

  @override
  String get telemetry => 'Telemetría';

  @override
  String get howLongTranscriptionTakesOnEach =>
      'Cuánto tarda la transcripción en cada dispositivo y su consumo de energía estimado.';

  @override
  String get logLevel => 'Nivel de registro';

  @override
  String get howMuchDetailTheAppWrites =>
      'Cuánto detalle escribe la app en sus registros. Depuración y Traza ayudan a diagnosticar problemas pero escriben más.';

  @override
  String get custom => 'Personalizado';

  @override
  String get shareLogs => 'Compartir registros';

  @override
  String get theLast7DaysOfDiagnostic =>
      'Los últimos 7 días de registros de diagnóstico, por ejemplo para adjuntarlos a un informe de error.';

  @override
  String get unableToReadTheAppLock =>
      'No se pudo leer el ajuste de bloqueo de la app.';

  @override
  String get needsAndroid10OrLater => 'Requiere Android 10 o posterior.';

  @override
  String get setAScreenLockInAndroid =>
      'Configura un bloqueo de pantalla en los ajustes de Android para usar el bloqueo de la app.';

  @override
  String askForYourFingerprintFaceOr(Object lockAfterBackground) {
    return 'Pide tu huella, tu cara o el bloqueo de pantalla cuando vuelvas a Koalino tras $lockAfterBackground segundos fuera. La grabación y la sincronización siguen funcionando mientras está bloqueado.';
  }

  @override
  String get otherNetworks => 'Otras redes';

  @override
  String get aRelayOnYourOwnAlways =>
      'Un relé en un dispositivo tuyo siempre encendido permite que tus dispositivos se encuentren lejos de la misma Wi-Fi. La sincronización en la misma red sigue funcionando sin él.';

  @override
  String get relayAddress => 'Dirección del relé';

  @override
  String get deviceAddressesOptionalOnePerLine =>
      'Direcciones de dispositivos (opcional, una por línea)';

  @override
  String get saveAddresses => 'Guardar direcciones';

  @override
  String get nothingThatWasSharedCouldBe =>
      'No se pudo leer nada de lo compartido.';

  @override
  String couldNotBeRead(Object message, Object failedCount) {
    return '$message; $failedCount no se pudieron leer';
  }

  @override
  String saveAsTheSummaryOf(Object title) {
    return 'Guardar como resumen de \"$title\"';
  }

  @override
  String get addToAProjectInstead => 'Añadir a un proyecto en su lugar';

  @override
  String get splitThisMeeting => '¿Separar esta reunión?';

  @override
  String itWasRecordedOnAndThe(Object devices, Object recordings) {
    return 'Se grabó en $devices y la app unió las grabaciones. Al separarla, la otra grabación ($recordings) vuelve a aparecer por su cuenta, con su propia transcripción, aquí y en tus dispositivos vinculados. No se volverán a unir. Esta reunión conserva su transcripción.';
  }

  @override
  String get unableToSplitThisMeeting => 'No se pudo separar esta reunión.';

  @override
  String get telemetryDashboard => 'Panel de telemetría';

  @override
  String get transcription => 'Transcripción';

  @override
  String get latency => 'Latencia';

  @override
  String get tasks => 'Tareas';

  @override
  String get power => 'Energía';

  @override
  String get transcriptionDurationOverTime =>
      'Duración de la transcripción a lo largo del tiempo';

  @override
  String get latencyDistribution => 'Distribución de la latencia';

  @override
  String get taskTypeBreakdown => 'Desglose por tipo de tarea';

  @override
  String get powerConsumptionEstimate => 'Estimación del consumo de energía';

  @override
  String get fastestLeastAccurate => 'El más rápido, el menos preciso';

  @override
  String get fastManyMistakesTheAppShips =>
      'Rápido, muchos errores. La app incluye este';

  @override
  String get goodBalanceOnMostComputers =>
      'Buen equilibrio en la mayoría de ordenadores';

  @override
  String get accurateSlowerNeedsAbout2Gb =>
      'Preciso, más lento, necesita unos 2 GB de memoria';

  @override
  String get olderNeedsAPowerfulComputer =>
      'Más antiguo, necesita un ordenador potente';

  @override
  String get mostAccurateNeedsAPowerfulComputer =>
      'El más preciso, necesita un ordenador potente';

  @override
  String get veryAccurateBestWithAGraphics =>
      'Muy preciso. Mejor con una tarjeta gráfica';

  @override
  String get thisModelMayBeTooLarge => 'Este modelo puede ser demasiado grande';

  @override
  String mayNeedMoreMemoryThanThis(Object model) {
    return '$model puede necesitar más memoria de la que tiene este dispositivo, y las grabaciones podrían fallar o ir lentas.';
  }

  @override
  String get downloadAnyway => 'Descargar de todos modos';

  @override
  String get transcriptionSettingsAreUnavailable =>
      'Los ajustes de transcripción no están disponibles.';

  @override
  String get downloadFromQualcomm => '¿Descargar de Qualcomm?';

  @override
  String get about360MbIsDownloadedFrom =>
      'Se descargan unos 360 MB de Qualcomm, que necesitan unos 460 MB de espacio:\n\n• El runtime QNN de Qualcomm, que permite a las apps usar la NPU. Tiene la licencia AI Stack License de Qualcomm, no la licencia de la app.\n• Whisper Small, compilado por Qualcomm AI Hub para la NPU de este teléfono.\n\nCada archivo se verifica antes de usarse. Tus grabaciones nunca salen del teléfono. Al descargar, aceptas los términos de Qualcomm.';

  @override
  String get acceptAndDownload => 'Aceptar y descargar';

  @override
  String get downloadingTheNpuFiles => 'Descargando los archivos de la NPU…';

  @override
  String get transcribeOnTheNpu => 'Transcribir en la NPU';

  @override
  String get thisPhoneHasAnAiChip =>
      'Este teléfono tiene un chip de IA (NPU) que puede ejecutar Whisper Small dejando libre el procesador: 30 segundos de habla tardan unos 2 segundos. Necesita una descarga de Qualcomm.';

  @override
  String get download => 'Descargar';

  @override
  String get runsWhisperSmallOnThePhone =>
      'Ejecuta Whisper Small en el chip de IA del teléfono, en lugar del modelo elegido abajo, también para la pasada posterior a la reunión salvo que tenga su propio modelo. Los tiempos de las palabras son aproximados.';

  @override
  String get removeTheNpuFiles => 'Eliminar los archivos de la NPU';

  @override
  String get alsoRecordTheComputerSSound =>
      'Grabar también el sonido del ordenador';

  @override
  String get transcribesWhatTheComputerPlaysSuch =>
      'Transcribe lo que reproduce el ordenador, como el otro lado de una llamada, junto al micrófono. Cuando el micrófono también oye los altavoces, las palabras repetidas se guardan una sola vez.';

  @override
  String get improveTheTranscriptAfterTheMeeting =>
      'Mejorar la transcripción después de la reunión';

  @override
  String get transcribesEachMeetingAgainWithMore =>
      'Vuelve a transcribir cada reunión con más contexto cuando termina; un teléfono espera a estar cargando. Hasta entonces el habla se guarda cifrada y después se borra.';

  @override
  String get modelForThisPass => 'Modelo para esta pasada';

  @override
  String get aLargerModelIsSlowerBut =>
      'Un modelo más grande es más lento pero más preciso, y después de la reunión hay tiempo. Descarga modelos arriba.';

  @override
  String get sameAsForRecording => 'El mismo que para grabar';

  @override
  String get keepSpeechForCombining => 'Conservar el habla para combinar';

  @override
  String get whenAMeetingWasAlsoRecorded =>
      'Cuando una reunión también se grabó en otro dispositivo, su habla espera este tiempo para combinarse y después se borra.';

  @override
  String get whisperModel => 'Modelo de Whisper';

  @override
  String get biggerModelsMakeFewerMistakesAnd =>
      'Los modelos más grandes cometen menos errores, y necesitan más tiempo y memoria. Las descargas vienen de Hugging Face. Esta elección solo se aplica a este dispositivo.';

  @override
  String automatic2(Object fileName, Object mode) {
    return 'Automático: $fileName$mode';
  }

  @override
  String usesUntilIsDownloadedTheBest(
      Object fileName, Object waiting, Object reason) {
    return 'Usa $fileName hasta que se descargue $waiting, el más adecuado para este dispositivo. $reason';
  }

  @override
  String picksTheBestModelForThis(Object fileName, Object mode) {
    return 'Elige el mejor modelo para este dispositivo: $fileName$mode.';
  }

  @override
  String download2(Object sizeBytes) {
    return 'Descargar $sizeBytes';
  }

  @override
  String tryLabel(Object model) {
    return 'Probar $model';
  }

  @override
  String get use => 'Usar';

  @override
  String suggestedForThisDevice(Object model, Object mode) {
    return 'Recomendado para este dispositivo: $model$mode';
  }

  @override
  String get efficientMode => 'Modo eficiente';

  @override
  String runsTheSameModelsOnThe(Object gpuName) {
    return 'Ejecuta los mismos modelos en el chip gráfico ($gpuName), normalmente más rápido. Desactivado usa el procesador, como de costumbre.';
  }

  @override
  String get theGraphicsChipStoppedTheApp =>
      'La última vez el chip gráfico detuvo la app, así que este dispositivo transcribe con su procesador.';

  @override
  String get tryTheGraphicsChipAgain => 'Volver a probar el chip gráfico';

  @override
  String get onThisDevice => 'en este dispositivo';

  @override
  String get usedByAutomatic => 'usado por Automático';

  @override
  String get englishOnly => 'Solo inglés';

  @override
  String get mayBeTooLargeForThis =>
      'puede ser demasiado grande para este dispositivo';

  @override
  String get doesnTMatchYourLanguage => 'no coincide con tu idioma';

  @override
  String get notEnoughFreeSpace => 'no hay suficiente espacio libre';

  @override
  String get cancelDownload => 'Cancelar descarga';

  @override
  String get glossary => 'Glosario';

  @override
  String get namesProjectNamesAndTermsOne =>
      'Nombres, nombres de proyectos y términos, uno por línea. La transcripción los escribe como aquí.';

  @override
  String get saveGlossary => 'Guardar glosario';

  @override
  String get voiceGallery => 'Galería de voces';

  @override
  String get tryAgain => 'Reintentar';

  @override
  String get unknownVoices => 'Voces desconocidas';

  @override
  String get noUnknownSpeakersToReview =>
      'No hay voces desconocidas que revisar';

  @override
  String get savedPeople => 'Personas guardadas';

  @override
  String get noSavedPeopleYetNameA =>
      'Aún no hay personas guardadas. Pon nombre a una voz para añadir a alguien.';

  @override
  String delete4(Object displayName) {
    return 'Eliminar $displayName';
  }

  @override
  String get playing => 'Reproduciendo…';

  @override
  String get untitledMeeting => 'Reunión sin título';

  @override
  String get play => 'Reproducir';

  @override
  String get deleteThisVoice => 'Eliminar esta voz';

  @override
  String theCallShowedSpeakingCheckThen(Object suggestedName) {
    return 'La llamada mostraba a $suggestedName hablando. Compruébalo y luego Asignar.';
  }

  @override
  String get chooseASavedPerson => 'Elige una persona guardada';

  @override
  String get assign => 'Asignar';

  @override
  String get deleteTheseVoices => 'Eliminar estas voces';

  @override
  String get theseSoundLikeOnePersonUntick =>
      'Parecen una sola persona. Desmarca las que sean de otra.';

  @override
  String itsVoiceSamplesGoForGood(Object voices) {
    return 'Sus $voices muestras de voz se borran para siempre, aquí y en tus dispositivos vinculados. Las transcripciones se conservan.';
  }

  @override
  String get itsVoiceSampleGoesForGood =>
      'Su muestra de voz se borra para siempre, aquí y en tus dispositivos vinculados. La transcripción se conserva.';

  @override
  String get theirNameAndVoiceSamplesGo =>
      'Su nombre y sus muestras de voz se borran para siempre, aquí y en tus dispositivos vinculados. Las transcripciones se conservan y lo numeran como hablante.';

  @override
  String unknownSpeaker(Object index) {
    return 'Hablante desconocido $index';
  }

  @override
  String attachedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count archivos adjuntados',
      one: '1 archivo adjuntado',
    );
    return '$_temp0';
  }

  @override
  String addedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count archivos añadidos',
      one: '1 archivo añadido',
    );
    return '$_temp0';
  }

  @override
  String emailAttachmentsLeft(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count adjuntos quedaron en el correo',
      one: '1 adjunto quedó en el correo',
    );
    return '$_temp0';
  }

  @override
  String addedMeetings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count reuniones añadidas',
      one: '1 reunión añadida',
    );
    return '$_temp0';
  }

  @override
  String otherRecordings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count grabaciones',
      one: '1 grabación',
    );
    return '$_temp0';
  }

  @override
  String get severalDevices => 'varios dispositivos';

  @override
  String meetingCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count reuniones',
      one: '1 reunión',
    );
    return '$_temp0';
  }

  @override
  String fileCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count archivos',
      one: '1 archivo',
    );
    return '$_temp0';
  }

  @override
  String chunkCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count fragmentos',
      one: '1 fragmento',
    );
    return '$_temp0';
  }

  @override
  String resultCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count resultados',
      one: '1 resultado',
    );
    return '$_temp0';
  }

  @override
  String get moreNotShown =>
      ', hay más sin mostrar: añade palabras para acotar';

  @override
  String cpuCores(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count núcleos de CPU',
      one: '1 núcleo de CPU',
    );
    return '$_temp0';
  }

  @override
  String dayCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count días',
      one: '1 día',
    );
    return '$_temp0';
  }

  @override
  String voiceSampleCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count muestras de voz',
      one: '1 muestra de voz',
      zero: 'aún sin muestra de voz, así que no se reconoce por la voz',
    );
    return '$_temp0';
  }

  @override
  String picturesIncluded(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'se incluyeron $count imágenes',
      one: 'se incluyó 1 imagen',
    );
    return '$_temp0';
  }

  @override
  String partCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count partes',
      one: '1 parte',
    );
    return '$_temp0';
  }

  @override
  String inMeetings(Object parts, Object meetings) {
    return '$parts en $meetings';
  }

  @override
  String assignedVoicesTo(int count, Object name) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count voces asignadas a $name',
      one: 'Asignado a $name',
    );
    return '$_temp0';
  }

  @override
  String addSharedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Añadir $count archivos compartidos a un proyecto',
      one: 'Añadir el archivo compartido a un proyecto',
    );
    return '$_temp0';
  }

  @override
  String keepSpeechSaved(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other:
          'Guardado. Ahora el habla espera hasta $count días para combinarse.',
      one: 'Guardado. Ahora el habla espera hasta 1 día para combinarse.',
    );
    return '$_temp0';
  }

  @override
  String titlesCanBeAtMost(Object max) {
    return 'Los títulos pueden tener como máximo $max caracteres.';
  }

  @override
  String promptUnknownPlaceholder(Object example) {
    return 'El prompt tiene un $example desconocido o sin cerrar.';
  }

  @override
  String get live => '● En directo';

  @override
  String get offline => '○ Sin conexión';

  @override
  String minutesShort(Object minutes) {
    return '$minutes min';
  }

  @override
  String get appLanguage => 'Idioma de la app';

  @override
  String get appLanguageDescription =>
      'El idioma de los menús y mensajes de Koalino.';

  @override
  String get systemLanguage => 'Predeterminado del sistema';

  @override
  String photosAddedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count fotos añadidas',
      one: '1 foto añadida',
    );
    return '$_temp0';
  }

  @override
  String moreModels(int count) {
    return 'Más modelos ($count)';
  }

  @override
  String get moreModelsHint =>
      'Versiones más ligeras que cometen algún error más, versiones solo en inglés y modelos que quizá no quepan en este dispositivo.';

  @override
  String get microphoneSilencedByCall =>
      'Android está silenciando el micrófono mientras otra app lo usa para una llamada: no se graba nada hasta que termine la llamada.';

  @override
  String get handoffInTheBrowser =>
      'Koalino abre el sitio web en tu navegador, donde has iniciado sesión con tu propia cuenta, y le pasa la reunión con tu prompt. La siguiente ventana muestra qué hacer allí.';

  @override
  String toKeepTheSummaryCopyTheWebsite(String appName) {
    return 'Para guardar el resumen, copia la respuesta de $appName en el sitio y vuelve a Koalino.';
  }

  @override
  String get useYourClaudeOrChatgptPlan => 'Usa tu plan de Claude o ChatGPT';

  @override
  String get summariesCanUseYourOwnPlan =>
      'Los resúmenes pueden usar tu propia suscripción de Claude o ChatGPT, sin clave de API. Instala Claude Code o Codex e inicia sesión una vez: después Koalino escribe los resúmenes por sí solo.';

  @override
  String get installClaudeCode => 'Instalar Claude Code';

  @override
  String get forAClaudePlanOpensTheGuide =>
      'Para un plan de Claude. Abre la guía de instalación.';

  @override
  String get installCodex => 'Instalar Codex';

  @override
  String get forAChatgptPlanOpensTheGuide =>
      'Para un plan de ChatGPT. Abre la guía de instalación.';

  @override
  String get skipUseTheWebsite => 'Omitir: usar el sitio web';

  @override
  String get koalinoCopiesTheMeetingAndOpens =>
      'Nada que instalar: Koalino copia la reunión y abre claude.ai o chatgpt.com, y tú la pegas allí.';

  @override
  String get addAConnection => 'Añadir una conexión';

  @override
  String get claudeCodeOrCodexAlreadyInstalled =>
      'Claude Code o Codex ya instalados, o un servidor de tu red.';

  @override
  String get noConnectionNeededUseTheWebsite =>
      'No hace falta ninguna conexión. En una reunión, abre Resumen y elige Usar tu propia app de IA…: Koalino copia la reunión y abre claude.ai o chatgpt.com.';

  @override
  String get afterInstallingAddTheConnection =>
      'Después de instalarlo e iniciar sesión, vuelve y elige Añadir conexión → Añadir una conexión → Claude Code o Codex.';

  @override
  String get claudeAiOrChatgptComInYourBrowser =>
      'claude.ai o chatgpt.com en tu navegador, con tu cuenta';

  @override
  String get newMeeting => 'Nueva reunión';

  @override
  String get newMeetingExplanation =>
      'Una reunión sin grabación. Escribe notas o añade archivos: los resúmenes y el chat los usan.';

  @override
  String get titleOptional => 'Título (opcional)';

  @override
  String handoffStepPressEnter(String site) {
    return '$site se abre con la reunión ya en el cuadro de mensaje. Pulsa Intro para enviarla.';
  }

  @override
  String handoffStepSentByItself(String site) {
    return '$site se abre y envía la reunión por sí solo. Inicia sesión primero si te lo pide.';
  }

  @override
  String handoffStepPaste(String site) {
    return '$site se abre y Koalino copia la reunión. Haz clic en el cuadro de mensaje, pégala (Ctrl+V) y pulsa Intro.';
  }

  @override
  String get handoffStepDragFiles =>
      'También se abre una carpeta con los archivos: arrástralos todos al cuadro del mensaje para que vayan como archivos.';

  @override
  String get handoffStepCopyAnswer =>
      'Cuando la respuesta esté completa, pulsa el botón de copiar debajo de ella.';

  @override
  String get handoffStepComeBack =>
      'Vuelve a Koalino: la respuesta se convierte en el resumen de la reunión, con Deshacer.';

  @override
  String get handoffStepAskThere =>
      'Luego haz allí tus preguntas sobre la reunión.';

  @override
  String openSite(String site) {
    return 'Abrir $site';
  }

  @override
  String writtenBySourceOn(String source, String createdAt) {
    return 'Escrito por $source el $createdAt';
  }

  @override
  String get noRecordingWriteNotesOrAttach =>
      'Sin grabación. Escribe notas o adjunta archivos: los resúmenes y el chat los usan.';

  @override
  String handoffStepOpensFilled(String site) {
    return '$site se abre con la reunión ya en el cuadro de mensaje.';
  }

  @override
  String handoffStepOpensCopied(String site) {
    return '$site se abre y Koalino copia la reunión.';
  }

  @override
  String get handoffStepSend => 'Pulsa Intro para enviarla.';

  @override
  String get handoffStepPasteSend =>
      'Haz clic en el cuadro de mensaje, pega la reunión (Ctrl+V) y pulsa Intro.';

  @override
  String get chatHandoffIntro =>
      'Haz tus preguntas en tu propia app de IA, con tu propia cuenta. Koalino le pasa las notas y las transcripciones, y las fotos y los archivos van con ellas como archivos.';

  @override
  String get plainTextFile => 'Texto sin formato (.txt)';

  @override
  String get unableToExportTheSummary => 'No se pudo exportar el resumen';

  @override
  String get addAVoice => 'Añadir una voz';

  @override
  String alreadySavedVoiceAddsToThem(String name) {
    return '$name ya está guardado: esta voz se le añadirá.';
  }

  @override
  String get addToASavedPerson => 'Añadir a una persona guardada';

  @override
  String addVoicesTo(String name) {
    return 'Añadir voces a $name';
  }

  @override
  String tickTheVoicesThatAre(String name) {
    return 'Estas voces no se reconocieron como de nadie. Marca las de $name; usa Reproducir para comprobarlo.';
  }

  @override
  String addTo(String name) {
    return 'Añadir a $name';
  }

  @override
  String get apiKey => 'Clave de API';

  @override
  String get onlyForOnlineServicesKeptIn =>
      'Solo para servicios en línea. Se guarda en el llavero de este dispositivo y solo se envía a esta dirección.';

  @override
  String get aKeyIsSavedLeaveEmpty =>
      'Hay una clave guardada. Déjalo vacío para conservarla.';

  @override
  String get removeTheSavedKey => 'Quitar la clave guardada';

  @override
  String get presetOllama =>
      'En tu ordenador o red: gratis, privado, funciona sin conexión (Gemma, Qwen, Llama…).';

  @override
  String get presetLmStudio =>
      'Como Ollama, con una app de escritorio para elegir modelos.';

  @override
  String get presetGemini =>
      'Google. Nivel gratuito con límites; en él Google puede usar lo que envías para mejorar sus productos. Clave: aistudio.google.com.';

  @override
  String get presetDeepseek =>
      'Pago por uso, muy barato, crédito prepago. Clave: platform.deepseek.com.';

  @override
  String get presetQwen =>
      'Alibaba. Pago por uso. Clave: Alibaba Cloud Model Studio.';

  @override
  String get presetKimi =>
      'Moonshot AI. Pago por uso. Clave: platform.moonshot.ai.';

  @override
  String get presetMistral =>
      'Mistral (Francia). Nivel gratuito con límites estrictos. Clave: console.mistral.ai.';

  @override
  String get presetOpenrouter =>
      'Una clave para muchos modelos (Gemini, DeepSeek, Qwen…), algunos gratis; el texto también pasa por OpenRouter. Clave: openrouter.ai.';

  @override
  String get keepMeetingRecordings => 'Guardar las grabaciones de reuniones';

  @override
  String get keepMeetingRecordingsSubtitle =>
      'Guarda el audio de cada reunión tras transcribirla, cifrado en este dispositivo, para reproducirlo. Desactivado por defecto.';

  @override
  String get keepMeetingRecordingsQuestion =>
      '¿Guardar las grabaciones de reuniones?';

  @override
  String get keepRecordingsWarning =>
      'Las grabaciones contienen las voces de todos los asistentes. Avisa a las personas que grabas y respeta las normas de consentimiento de tu país: en muchos lugares grabar a otros requiere su permiso. Las grabaciones se quedan cifradas en este dispositivo y no se sincronizan; ocupan alrededor de 2 MB por cada 10 minutos de voz. Se eliminan con su reunión, o todas a la vez con Borrar todas las grabaciones. Solo se guarda la voz, así que los tramos en silencio se oyen como silencio.';

  @override
  String get keepRecordings => 'Guardar grabaciones';

  @override
  String recordingsKept(int count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count grabaciones guardadas · $size MB',
      one: '1 grabación guardada · $size MB',
      zero: 'Ninguna grabación guardada',
    );
    return '$_temp0';
  }

  @override
  String get wipeAllRecordings => 'Borrar todas las grabaciones';

  @override
  String get wipeAllRecordingsQuestion => '¿Borrar todas las grabaciones?';

  @override
  String get wipeAllRecordingsExplanation =>
      'Se elimina cada grabación guardada en este dispositivo. Las transcripciones y notas se quedan. No se puede deshacer.';

  @override
  String recordingsWiped(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count grabaciones borradas.',
      one: '1 grabación borrada.',
      zero: 'No hay grabaciones que borrar.',
    );
    return '$_temp0';
  }

  @override
  String get recordingsUnavailable =>
      'Las grabaciones no están disponibles ahora.';

  @override
  String get playRecording => 'Reproducir la grabación';

  @override
  String get stopRecordingPlayback => 'Detener la reproducción';

  @override
  String get recordingCouldNotBePlayed => 'No se pudo reproducir la grabación.';

  @override
  String get columnWidth => 'Ancho de la columna';

  @override
  String get subprojects => 'Subproyectos';

  @override
  String get newSubproject => 'Nuevo subproyecto';

  @override
  String get moveProjectTo => 'Mover a…';

  @override
  String get topLevel => 'Nivel superior';

  @override
  String moveProjectTitle(String project) {
    return 'Mover $project a';
  }

  @override
  String get movedToTopLevel => 'Movido al nivel superior';

  @override
  String get theProjectCouldNotBeMoved => 'No se pudo mover el proyecto.';

  @override
  String subprojectCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count subproyectos',
      one: '1 subproyecto',
    );
    return '$_temp0';
  }

  @override
  String get chooseAProject =>
      'Elige un proyecto para ver sus reuniones y archivos.';

  @override
  String get chooseAMeetingOrFile =>
      'Elige una reunión o un archivo para verlo aquí.';

  @override
  String get backgroundPassTitle =>
      'Mejorar las transcripciones con Koalino cerrado';

  @override
  String get backgroundPassAllowed =>
      'Permitido: el teléfono mejora las transcripciones pendientes cuando se carga, incluso con Koalino cerrado o el ahorro de energía activado.';

  @override
  String get backgroundPassAsk =>
      'Toca para que Android lo haga mientras el teléfono se carga, incluso con el ahorro de energía activado. Si no, solo ocurre con Koalino abierto.';

  @override
  String get chatPrompt => 'Prompt del chat';

  @override
  String chatPromptHelp(String placeholder) {
    return '$placeholder representa aquello sobre lo que preguntas: una reunión, un proyecto o todas tus notas. Las respuestas siempre citan sus fuentes con un número, y tus notas van después del prompt.';
  }

  @override
  String get saveChatPrompt => 'Guardar prompt del chat';

  @override
  String get chatPromptSaved => 'Prompt del chat guardado.';

  @override
  String get theBuiltInChatPromptIsBack =>
      'Ha vuelto el prompt del chat predeterminado.';

  @override
  String get promptsSharedWithPairedDevices =>
      'Ambos prompts son iguales en todos tus dispositivos vinculados: un cambio aquí les llega cuando se conectan.';
}
