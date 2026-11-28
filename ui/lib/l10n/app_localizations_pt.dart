// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Portuguese (`pt`).
class AppLocalizationsPt extends AppLocalizations {
  AppLocalizationsPt([String locale = 'pt']) : super(locale);

  @override
  String get unableToCheckTheAppLock =>
      'Não foi possível verificar o bloqueio do app. Tente novamente.';

  @override
  String get unableToShowTheUnlockPrompt =>
      'Não foi possível mostrar o pedido de desbloqueio. Tente novamente.';

  @override
  String get setAScreenLockOnThis =>
      'Configure primeiro um bloqueio de tela neste dispositivo.';

  @override
  String get unableToChangeTheAppLock =>
      'Não foi possível alterar o bloqueio do app. Tente novamente.';

  @override
  String get aCallApp => 'Um app de chamadas';

  @override
  String get call => 'Chamada';

  @override
  String call2(Object app) {
    return 'Chamada do $app';
  }

  @override
  String theCallEndedSoTheRecording(Object tiedApp) {
    return 'A chamada do $tiedApp terminou, então a gravação parou.';
  }

  @override
  String get restartYourBrowserAndCallApps =>
      'Reinicie uma vez o navegador e os apps de chamada, para que o Koalino possa ler quem está falando nas chamadas.';

  @override
  String get unableToLoadDevices => 'Não foi possível carregar os dispositivos';

  @override
  String get thisDeviceIsNotOnA =>
      'Este dispositivo ainda não está em uma rede. Conecte-o e tente novamente.';

  @override
  String get thatDidNotWorkTryAgain => 'Não funcionou. Tente novamente.';

  @override
  String get use1To64CharactersWithout =>
      'Use de 1 a 64 caracteres, sem quebras de linha.';

  @override
  String get theNameCouldNotBeSaved =>
      'Não foi possível salvar o nome. Tente novamente.';

  @override
  String get thisInviteHasExpiredMakeA =>
      'Este convite expirou. Crie um novo no outro dispositivo.';

  @override
  String get thisInviteWasMadeOnThis =>
      'Este convite foi criado neste dispositivo. Use-o no outro.';

  @override
  String get thatDeviceIsAlreadyPaired => 'Esse dispositivo já está pareado.';

  @override
  String get thatIsNotAValidInvite =>
      'Este não é um convite válido. Copie-o novamente.';

  @override
  String get deviceSyncIsNotRunning =>
      'A sincronização de dispositivos não está ativa.';

  @override
  String get theOtherDeviceIsNoLonger =>
      'O outro dispositivo não está mais pareando. Tente novamente.';

  @override
  String get theOtherAppCouldNotBe => 'Não foi possível abrir o outro app.';

  @override
  String get theOtherApp => 'o outro app';

  @override
  String app(Object displayName) {
    return 'app $displayName';
  }

  @override
  String get unableToChangeTheLogLevel =>
      'Não foi possível alterar o nível de log';

  @override
  String logsExportedTo(Object filePath) {
    return 'Logs exportados para $filePath';
  }

  @override
  String get unableToShareTheLogs => 'Não foi possível compartilhar os logs';

  @override
  String get noLogsToExportYet => 'Ainda não há logs para exportar';

  @override
  String get unableToExportTheLogs => 'Não foi possível exportar os logs';

  @override
  String couldNotBeAttached(Object attached, Object failedPaths) {
    return '$attached; $failedPaths não puderam ser anexados';
  }

  @override
  String get theFilesCouldNotBeAttached =>
      'Não foi possível anexar os arquivos.';

  @override
  String get unableToShareTheExport =>
      'Não foi possível compartilhar a exportação';

  @override
  String exportedTo(Object filePath) {
    return 'Exportado para $filePath';
  }

  @override
  String get exportSaved => 'Exportação salva';

  @override
  String get unableToSaveTheExport => 'Não foi possível salvar a exportação';

  @override
  String get thisMeetingNoLongerExists => 'Esta reunião não existe mais';

  @override
  String get exportIsUnavailableUntilTheApp =>
      'A exportação fica indisponível até o app iniciar';

  @override
  String get unableToExportTheMeeting => 'Não foi possível exportar a reunião';

  @override
  String get unableToLoadMeetings => 'Não foi possível carregar as reuniões';

  @override
  String get unableToLoadThisMeeting =>
      'Não foi possível carregar esta reunião';

  @override
  String get enterATitle => 'Digite um título.';

  @override
  String titlesCanBeAtMostCharacters(
      Object maxMeetingTitleCharacters, Object runes) {
    return 'Os títulos podem ter no máximo $maxMeetingTitleCharacters caracteres (você tem $runes).';
  }

  @override
  String get titlesCanTContainLineBreaks =>
      'Os títulos não podem conter quebras de linha nem caracteres de controle.';

  @override
  String get notPairedYetMakeTheRelay =>
      'Ainda não pareado. Torne o relay visível e depois pareie-o em Dispositivos próximos, em Dispositivos.';

  @override
  String get connectingToTheRelay => 'Conectando ao relay…';

  @override
  String get connectedToTheRelayAskingIt =>
      'Conectado ao relay. Pedindo que aceite conexões…';

  @override
  String get connectedYourOtherDevicesCanReach =>
      'Conectado. Seus outros dispositivos podem alcançar este pelo relay.';

  @override
  String get savedThisAppliesNowWithNo => 'Salvo. Vale agora, sem reiniciar.';

  @override
  String get thatRelayAddressIsNotValid =>
      'Esse endereço de relay não é válido. Ele tem esta forma: /ip4/203.0.113.5/tcp/4001/p2p/12D3Koo…';

  @override
  String get aDeviceAddressIsNotValid =>
      'Um endereço de dispositivo não é válido. Cada um termina em /p2p/ e o ID do dispositivo.';

  @override
  String get unableToSaveTheAddresses =>
      'Não foi possível salvar os endereços.';

  @override
  String get thePhotoCouldNotBeSaved => 'Não foi possível salvar a foto.';

  @override
  String get enterAName => 'Digite um nome.';

  @override
  String namesCanBeAtMostCharacters(Object maxMeetingTitleCharacters) {
    return 'Os nomes podem ter no máximo $maxMeetingTitleCharacters caracteres.';
  }

  @override
  String get namesCanTContainLineBreaks =>
      'Os nomes não podem conter quebras de linha nem caracteres de controle.';

  @override
  String get unableToLoadProjects => 'Não foi possível carregar os projetos';

  @override
  String get unableToLoadThisProject =>
      'Não foi possível carregar este projeto';

  @override
  String get theFilesCouldNotBeAdded =>
      'Não foi possível adicionar os arquivos.';

  @override
  String couldNotBeAdded(Object failedPaths) {
    return '$failedPaths não puderam ser adicionados';
  }

  @override
  String get unableToStartRecording => 'Não foi possível iniciar a gravação';

  @override
  String get unableToPauseRecording => 'Não foi possível pausar a gravação';

  @override
  String get unableToResumeRecording => 'Não foi possível retomar a gravação';

  @override
  String get unableToStopRecording => 'Não foi possível parar a gravação';

  @override
  String get recordingSaved => 'Gravação salva';

  @override
  String get microphoneAccessIsNeededToRecord =>
      'É preciso acesso ao microfone para gravar';

  @override
  String get keepTheAppOpenAndroidDid =>
      'Mantenha o app aberto: o Android não permitiu gravar em segundo plano';

  @override
  String get unableToOpenTheMicrophone => 'Não foi possível abrir o microfone';

  @override
  String saved(Object label) {
    return '$label salva.';
  }

  @override
  String removed(Object label) {
    return '$label removida.';
  }

  @override
  String isNowTheDefault(Object id) {
    return '$id agora é a padrão.';
  }

  @override
  String answered(Object id, Object reply) {
    return '$id respondeu: $reply';
  }

  @override
  String get promptSaved => 'Prompt salvo.';

  @override
  String get theBuiltInPromptIsBack => 'O prompt integrado foi restaurado.';

  @override
  String get finishedMeetingsWillBeSummarizedAutomatically =>
      'As reuniões encerradas serão resumidas automaticamente.';

  @override
  String get automaticSummariesAreOff =>
      'Os resumos automáticos estão desativados.';

  @override
  String get thisComputerNoLongerWritesSummaries =>
      'Este computador não escreve mais resumos nem responde a chats para seus outros dispositivos.';

  @override
  String nowWritesTheSummariesAndAnswers(Object id) {
    return '$id agora escreve os resumos e responde aos chats que seus outros dispositivos pedem.';
  }

  @override
  String get requestCancelled => 'Solicitação cancelada.';

  @override
  String get thisMeetingUsesTheSettingsPrompt =>
      'Esta reunião volta a usar o prompt das Configurações.';

  @override
  String get promptSavedForThisMeeting => 'Prompt salvo para esta reunião.';

  @override
  String get summarySaved => 'Resumo salvo.';

  @override
  String get theConnection => 'A conexão';

  @override
  String get summaryWritten => 'Resumo escrito.';

  @override
  String theTranscriptWasReadInParts(Object parts) {
    return ' A transcrição foi lida em $parts partes.';
  }

  @override
  String leftOut(Object sent, Object imagesSkipped) {
    return ' $sent, $imagesSkipped deixadas de fora.';
  }

  @override
  String get theModelStoppedBeforeTheEnd => ' O modelo parou antes do fim.';

  @override
  String get waitingForYourComputerItWrites =>
      'Aguardando seu computador. Ele escreverá o resumo quando estiver ligado, conectado a este dispositivo e configurado para responder em Configurações → Conexões de IA.';

  @override
  String get yourComputerIsWritingTheSummary =>
      'Seu computador está escrevendo o resumo…';

  @override
  String yourComputerCouldNotWriteThe(Object message) {
    return 'Seu computador não conseguiu escrever o resumo: $message';
  }

  @override
  String get unableToRefreshTelemetry =>
      'Não foi possível atualizar a telemetria';

  @override
  String get automatic => 'Automático';

  @override
  String get transcriptionLanguage => 'Idioma da transcrição';

  @override
  String get transcriptionLanguages => 'Idiomas da transcrição';

  @override
  String get theLanguageCouldNotBeChanged => 'Não foi possível mudar o idioma.';

  @override
  String get thisDevice => 'Este dispositivo';

  @override
  String get bestAvailable => 'O melhor disponível';

  @override
  String get aRemovedDevice => 'Um dispositivo removido';

  @override
  String get theTranscriptionDeviceCouldNotBe =>
      'Não foi possível mudar o dispositivo de transcrição. Verifique se ele ainda está pareado.';

  @override
  String get downloadFinishedYouCanNowChoose =>
      'Download concluído. Agora você pode escolher este modelo.';

  @override
  String get downloadCancelled => 'Download cancelado.';

  @override
  String get theDownloadStoppedNoConnectionTo =>
      'O download parou: sem conexão com o servidor. Seu modelo atual continua funcionando.';

  @override
  String get theDownloadedFileFailedItsCheck =>
      'O arquivo baixado não passou na verificação e foi excluído. Seu modelo atual continua funcionando.';

  @override
  String get theModelCouldNotBeSaved =>
      'Não foi possível salvar o modelo. Verifique o espaço livre. Seu modelo atual continua funcionando.';

  @override
  String get theDownloadServerRefusedTheFile =>
      'O servidor de download recusou o arquivo. Seu modelo atual continua funcionando.';

  @override
  String get savedTheNextRecordingUsesThis =>
      'Salvo. A próxima gravação usará este modelo.';

  @override
  String get savedTheNextRecordingUsesThis2 =>
      'Salvo. A próxima gravação usará isto.';

  @override
  String get savedTheNextRecordingUsesThe =>
      'Salvo. A próxima gravação usará a escolha automática.';

  @override
  String get savedMeetingsRecordedFromNowOn =>
      'Salvo. As reuniões gravadas a partir de agora usarão isto.';

  @override
  String get savedTheNextRecordingUsesThese =>
      'Salvo. A próxima gravação usará estes nomes e termos.';

  @override
  String get eachTermCanHaveAtMost =>
      'Cada termo pode ter no máximo 60 caracteres, e todos juntos no máximo 500.';

  @override
  String get chooseAWindowBetween1And =>
      'Escolha um período entre 1 e 60 dias.';

  @override
  String get theDownloadCouldNotStartPlease =>
      'Não foi possível iniciar o download. Tente novamente.';

  @override
  String get savedTheNextRecordingIsTranscribed =>
      'Salvo. A próxima gravação será transcrita na NPU.';

  @override
  String get savedTheNextRecordingIsTranscribed2 =>
      'Salvo. A próxima gravação será transcrita no processador.';

  @override
  String get theNpuFilesAreNotAll =>
      'Os arquivos da NPU ainda não foram todos baixados.';

  @override
  String get theNpuFilesWereRemoved => 'Os arquivos da NPU foram removidos.';

  @override
  String get theNpuFilesCouldNotBe =>
      'Não foi possível remover os arquivos da NPU. Tente novamente.';

  @override
  String get downloadedRecordingsNowTranscribeOnThe =>
      'Baixado. As gravações agora são transcritas na NPU.';

  @override
  String get theDownloadStoppedNoConnectionTo2 =>
      'O download parou: sem conexão com o servidor.';

  @override
  String get aDownloadedFileFailedItsCheck =>
      'Um arquivo baixado não passou na verificação e foi excluído.';

  @override
  String get theFilesCouldNotBeSaved =>
      'Não foi possível salvar os arquivos. Verifique o espaço livre (são necessários cerca de 800 MB).';

  @override
  String get theDownloadServerRefusedTheFiles =>
      'O servidor de download recusou os arquivos.';

  @override
  String get theNextRecordingTriesTheGraphics =>
      'A próxima gravação tentará o chip gráfico de novo.';

  @override
  String get thatModelWonTBeProposed =>
      'Esse modelo não será sugerido de novo.';

  @override
  String get modelRemoved => 'Modelo removido.';

  @override
  String get anotherModelIsDownloadingWaitFor =>
      'Outro modelo está sendo baixado. Aguarde terminar.';

  @override
  String get thereIsnTEnoughFreeSpace =>
      'Não há espaço livre suficiente para este modelo.';

  @override
  String get thatDidnTWorkPleaseTry => 'Não funcionou. Tente novamente.';

  @override
  String get unableToLoadVoiceGallery =>
      'Não foi possível carregar a Galeria de vozes';

  @override
  String get unableToPlayThisVoice => 'Não foi possível reproduzir esta voz';

  @override
  String get unableToDeleteThisVoice => 'Não foi possível excluir esta voz';

  @override
  String get voiceDeleted => 'Voz excluída';

  @override
  String get unableToDeleteThisPerson => 'Não foi possível excluir esta pessoa';

  @override
  String deleted(Object displayName) {
    return '$displayName excluído';
  }

  @override
  String get enterANameBeforeAssigning => 'Digite um nome antes de atribuir';

  @override
  String assignedTo(Object identityName) {
    return 'Atribuído a $identityName';
  }

  @override
  String assignedToThereWasNotEnough(Object identityName) {
    return 'Atribuído a $identityName. Não havia fala clara suficiente para reconhecer esta voz em gravações futuras.';
  }

  @override
  String get unableToAssignTheSpeakerCluster =>
      'Não foi possível atribuir o grupo de falas';

  @override
  String get chooseAtLeastOneVoiceTo =>
      'Escolha pelo menos uma voz para nomear';

  @override
  String thereWasNotEnoughClearSpeech(Object assigned) {
    return '$assigned. Não havia fala clara suficiente para reconhecer esta voz em gravações futuras.';
  }

  @override
  String get theSettingCouldNotBeSaved =>
      'Não foi possível salvar a configuração.';

  @override
  String get theAppIsStillStartingTry =>
      'O app ainda está iniciando. Tente novamente.';

  @override
  String get somethingWentWrongTryAgain => 'Algo deu errado. Tente novamente.';

  @override
  String isUsingTheMicrophone(Object app) {
    return '$app está usando o microfone';
  }

  @override
  String get recordThisCall => 'Gravar esta chamada?';

  @override
  String theAiServerRejectedTheRequest(Object detail) {
    return 'O servidor de IA rejeitou a solicitação.\nO servidor disse: $detail';
  }

  @override
  String claudeCodeOrCodexCouldNot(Object detail) {
    return 'O Claude Code ou o Codex não conseguiu responder.\nEle disse: $detail';
  }

  @override
  String get typeAQuestion => 'Digite uma pergunta.';

  @override
  String get theQuestionIsTooLong => 'A pergunta é longa demais.';

  @override
  String get addAnAiConnectionInSettings =>
      'Adicione uma conexão de IA nas Configurações ou escolha uma padrão.';

  @override
  String get thatAiConnectionNoLongerExists =>
      'Essa conexão de IA não existe mais.';

  @override
  String get thereIsNothingToTalkAbout =>
      'Ainda não há sobre o que conversar: nenhuma transcrição, resumo, nota ou arquivo.';

  @override
  String get theAiConnectionSContextIs =>
      'O contexto da conexão de IA é pequeno demais. Escolha uma com contexto maior.';

  @override
  String get thatMeetingNoLongerExists => 'Essa reunião não existe mais.';

  @override
  String get thatProjectNoLongerExists => 'Esse projeto não existe mais.';

  @override
  String get theAiServerCouldNotBe =>
      'Não foi possível alcançar o servidor de IA. Verifique a conexão.';

  @override
  String get theAiServerRefusedTheApi =>
      'O servidor de IA recusou a chave de API.';

  @override
  String get theAiServerIsBusyTry =>
      'O servidor de IA está ocupado. Tente mais tarde.';

  @override
  String get theModelDeclinedToAnswer => 'O modelo se recusou a responder.';

  @override
  String get claudeCodeOrCodexIsNot =>
      'O Claude Code ou o Codex não está instalado neste computador, ou o app não o encontra.';

  @override
  String get theNotesCouldNotBeRead => 'Não foi possível ler as notas.';

  @override
  String get noPairedComputerAnsweredOnA =>
      'Nenhum computador pareado respondeu. Em um computador, abra Configurações → IA, escolha uma conexão em \"Resumos para meus outros dispositivos\" e mantenha-o ligado e conectado.';

  @override
  String get theComputerTookTooLongTo =>
      'O computador demorou demais para responder.';

  @override
  String get theComputerDisconnectedBeforeAnsweringTry =>
      'O computador se desconectou antes de responder. Tente novamente.';

  @override
  String get theComputerAnswersAtMost30 =>
      'O computador responde no máximo 30 perguntas por hora. Tente mais tarde.';

  @override
  String get theComputerIsAnsweringOtherQuestions =>
      'O computador está respondendo outras perguntas. Tente daqui a pouco.';

  @override
  String get pairedDevicesAreOffOnThis =>
      'Os dispositivos pareados estão desativados neste telefone.';

  @override
  String get theAiServerCouldNotAnswer =>
      'O servidor de IA não conseguiu responder. Tente novamente.';

  @override
  String get exportMeeting => 'Exportar reunião';

  @override
  String get openaiCompatibleServer => 'Servidor compatível com OpenAI';

  @override
  String get localServerOllama => 'Servidor local (Ollama)';

  @override
  String get claudeCodeClaudePlan => 'Claude Code (plano Claude)';

  @override
  String get codexChatgptPlan => 'Codex (plano ChatGPT)';

  @override
  String get checkTheAddressAndModelPlain =>
      'Verifique o endereço e o modelo. O http:// simples só funciona neste dispositivo ou na rede local.';

  @override
  String get thatConnectionNoLongerExists => 'Essa conexão não existe mais.';

  @override
  String get addAnAiConnectionInSettings2 =>
      'Adicione primeiro uma conexão de IA nas Configurações.';

  @override
  String get thisMeetingHasNoTranscriptOr =>
      'Esta reunião ainda não tem transcrição nem notas.';

  @override
  String get thisMeetingIsTooLongFor =>
      'Esta reunião é longa demais para esta conexão. Escolha uma com contexto maior.';

  @override
  String get theAiServerCouldNotBe2 =>
      'Não foi possível alcançar o servidor de IA.';

  @override
  String get theAiServerRefusedAccessUse =>
      'O servidor de IA recusou o acesso. Verifique a chave de API da conexão.';

  @override
  String get theAiServerIsBusyOr =>
      'O servidor de IA está ocupado ou acima do limite. Tente mais tarde.';

  @override
  String get theAiServerRejectedTheRequest2 =>
      'O servidor de IA rejeitou a solicitação. Verifique o nome do modelo.';

  @override
  String get theAiServerFailedTryAgain =>
      'O servidor de IA falhou. Tente novamente.';

  @override
  String get theAiServerSAnswerCould =>
      'Não foi possível ler a resposta do servidor de IA.';

  @override
  String get claudeCodeOrCodexCouldNot2 =>
      'O Claude Code ou o Codex não conseguiu responder. Verifique se ele está conectado à conta.';

  @override
  String get theSettingsCouldNotBeSaved =>
      'Não foi possível salvar as configurações.';

  @override
  String get theMeetingCouldNotBeRead =>
      'Não foi possível ler ou salvar a reunião.';

  @override
  String get error => 'Erro';

  @override
  String get warning => 'Aviso';

  @override
  String get infoDefault => 'Info (padrão)';

  @override
  String get debug => 'Depuração';

  @override
  String get trace => 'Rastreamento';

  @override
  String get thisDevice2 => 'este dispositivo';

  @override
  String get anotherDevice => 'outro dispositivo';

  @override
  String notConnected(Object label) {
    return '$label (não conectado)';
  }

  @override
  String itIsKeptEncryptedUntil(Object until) {
    return ' Fica guardado criptografado até $until.';
  }

  @override
  String get recordedOnThisDeviceOnly => 'Gravada apenas neste dispositivo.';

  @override
  String recordedOnThisDeviceOnlyA(Object until) {
    return 'Gravada apenas neste dispositivo. Uma gravação dela feita em outro dispositivo ainda pode ser combinada até $until.';
  }

  @override
  String combinesTheRecordingsNothingIsLeft(Object combiner) {
    return '$combiner combina as gravações; não há mais nada a enviar daqui.';
  }

  @override
  String waitingForAudioFrom(Object names, Object kept) {
    return 'Aguardando o áudio de $names.$kept';
  }

  @override
  String combinesTheRecordingsWaitingForAudio(Object combiner, Object names) {
    return '$combiner combina as gravações; aguardando o áudio de $names.';
  }

  @override
  String get audioFromEveryDeviceIsHere =>
      'O áudio de todos os dispositivos chegou; a transcrição é combinada quando este dispositivo estiver livre.';

  @override
  String thisDeviceSAudioStillHas(Object combiner, Object kept) {
    return 'O áudio deste dispositivo ainda precisa chegar a $combiner.$kept';
  }

  @override
  String get transcriptCombinedFromEveryDevice =>
      'Transcrição combinada de todos os dispositivos.';

  @override
  String notCombinedAudioFromDidNot(Object plainNames) {
    return 'Não combinada: o áudio de $plainNames não chegou em três semanas.';
  }

  @override
  String meeting(Object createdAt) {
    return 'Reunião $createdAt';
  }

  @override
  String get writeSomethingFirst => 'Escreva algo primeiro.';

  @override
  String get theTextIsTooLong => 'O texto é longo demais.';

  @override
  String get thatPartOfTheTranscriptChanged =>
      'Essa parte da transcrição mudou nesse meio-tempo. Abra a reunião de novo.';

  @override
  String get theMeetingCouldNotBeSaved => 'Não foi possível salvar a reunião.';

  @override
  String get chooseAFullFolderPathInside =>
      'Escolha o caminho completo de uma pasta, dentro da sua pasta pessoal, mas não a própria pasta pessoal.';

  @override
  String get theFolderCouldNotBeWritten =>
      'Não foi possível gravar na pasta. Verifique se ela existe e se você pode gravar nela.';

  @override
  String get theNotesCouldNotBeRead2 =>
      'Não foi possível ler ou salvar as notas.';

  @override
  String get theInviteAnswerIsIncomplete =>
      'A resposta ao convite está incompleta.';

  @override
  String get theRenameAnswerIsIncomplete =>
      'A resposta à renomeação está incompleta.';

  @override
  String get thisDeviceHasNoCamera => 'Este dispositivo não tem câmera.';

  @override
  String get theCameraIsNotOpen => 'A câmera não está aberta.';

  @override
  String get title => 'Título';

  @override
  String get summary => 'Resumo';

  @override
  String get notes => 'Notas';

  @override
  String get transcript => 'Transcrição';

  @override
  String get projectName => 'Nome do projeto';

  @override
  String file(Object fileName) {
    return 'Arquivo $fileName';
  }

  @override
  String get typeWhatToLookFor => 'Digite o que procurar.';

  @override
  String get typeAtLeastTwoLetters => 'Digite pelo menos duas letras.';

  @override
  String get theSearchIsTooLong => 'A pesquisa é longa demais.';

  @override
  String get exportLogs => 'Exportar logs';

  @override
  String about(Object appName) {
    return 'Sobre o $appName';
  }

  @override
  String get versionLicenseAndTheSoftwareIt =>
      'Versão, licença e o software que ele usa.';

  @override
  String remove(Object label) {
    return 'Remover $label?';
  }

  @override
  String get itsApiKeyIsDeletedFrom =>
      'A chave de API dela é apagada deste dispositivo. Os resumos já escritos permanecem.';

  @override
  String get cancel => 'Cancelar';

  @override
  String get remove2 => 'Remover';

  @override
  String get aiConnections => 'Conexões de IA';

  @override
  String get addConnection => 'Adicionar conexão';

  @override
  String get summariesSendTheMeetingSTranscript =>
      'Os resumos enviam a transcrição, as notas e as fotos da reunião ao servidor da conexão. Um servidor na sua rede local os mantém lá.';

  @override
  String get noConnectionsYet => 'Ainda não há conexões';

  @override
  String get addAServerOnYourNetwork =>
      'Adicione um servidor da sua rede (Ollama, LM Studio, llama.cpp), ou o Claude Code ou o Codex neste computador.';

  @override
  String get summarizeMeetingsAutomatically =>
      'Resumir reuniões automaticamente';

  @override
  String get withTheDefaultConnectionWhenA =>
      'Com a conexão padrão, quando a transcrição de uma reunião é definitiva.';

  @override
  String get summariesForMyOtherDevices =>
      'Resumos para meus outros dispositivos';

  @override
  String get aPairedPhoneCanAskThis =>
      'Um telefone pareado pode pedir a este computador que escreva um resumo ou responda a um chat com esta conexão, a partir da cópia das suas notas neste computador. Isso conta no seu plano; no máximo 10 resumos e 30 respostas de chat por hora.';

  @override
  String get off => 'Desativado';

  @override
  String get summaryPrompt => 'Prompt do resumo';

  @override
  String placeholdersTheTranscriptIsAlwaysSent(Object placeholders) {
    return 'Variáveis: $placeholders. A transcrição é sempre enviada depois do prompt.';
  }

  @override
  String get savePrompt => 'Salvar prompt';

  @override
  String get resetToBuiltIn => 'Restaurar o integrado';

  @override
  String get defaultLabel => 'Padrão';

  @override
  String get installed => 'Instalado';

  @override
  String get notFoundOnThisComputer => 'Não encontrado neste computador';

  @override
  String get unencryptedHttpLocalNetworkOnly =>
      'http sem criptografia, só rede local';

  @override
  String get runsWithYourPlanSSign =>
      'Usa o login do seu plano neste computador';

  @override
  String actionsFor(Object label) {
    return 'Ações para $label';
  }

  @override
  String get makeDefault => 'Tornar padrão';

  @override
  String get test => 'Testar';

  @override
  String get edit => 'Editar';

  @override
  String get enterANameAnAddressAnd =>
      'Digite um nome, um endereço e um modelo.';

  @override
  String get theContextIsANumberOf =>
      'O contexto é um número de tokens, como 8192.';

  @override
  String edit2(Object label) {
    return 'Editar $label';
  }

  @override
  String get name => 'Nome';

  @override
  String get protocol => 'Protocolo';

  @override
  String get runsTheToolInstalledAndSigned =>
      'Usa a ferramenta instalada e conectada neste computador, assim seu plano Claude ou ChatGPT escreve os resumos em vez de crédito de API. A reunião vai para a Anthropic ou a OpenAI com essa conta.';

  @override
  String get address => 'Endereço';

  @override
  String get httpsAlwaysHttpOnlyOnThis =>
      'https:// sempre; http:// só neste dispositivo ou na rede local.';

  @override
  String get model => 'Modelo';

  @override
  String get leaveEmptyForTheToolS =>
      'Deixe vazio para o padrão da própria ferramenta.';

  @override
  String get contextTokens => 'Contexto (tokens)';

  @override
  String get saveConnection => 'Salvar conexão';

  @override
  String get koalinoIsLocked => 'O Koalino está bloqueado';

  @override
  String get unlockWithYourFingerprintFaceOr =>
      'Desbloqueie com sua digital, seu rosto ou o bloqueio de tela.';

  @override
  String get unlock => 'Desbloquear';

  @override
  String copied(Object what) {
    return '$what copiado';
  }

  @override
  String get aiAssistantAccess => 'Acesso do assistente de IA';

  @override
  String get letYourOwnAiAssistantSuch =>
      'Deixe seu próprio assistente de IA, como Claude Code, Codex ou Claude Desktop, ler suas reuniões e projetos: títulos, resumos, notas, transcrições e arquivos de projetos. Ele só pode ler; não pode alterar nem excluir nada.';

  @override
  String get whatTheAssistantReadsMayBe =>
      'O que o assistente lê pode ser enviado ao modelo on-line dele, na sua conta desse serviço. Ative isto só para um assistente a quem você confiaria suas reuniões.';

  @override
  String get theAssistantServerIsNotInstalled =>
      'O servidor do assistente não está instalado com esta cópia do app.';

  @override
  String get letMyAiAssistantReadMy =>
      'Deixar meu assistente de IA ler minhas notas';

  @override
  String get connectYourAssistant => 'Conecte seu assistente';

  @override
  String get yourNotesAreUnlockedWithA =>
      'Suas notas são desbloqueadas com uma frase-senha, que nunca é escrita nas configurações de um assistente. O servidor só as lê se NOTE_TAKER_ENCRYPTION_PASSPHRASE estiver definida onde o assistente o inicia.';

  @override
  String get claudeCodeRunInATerminal => 'Claude Code: execute em um terminal';

  @override
  String get command => 'Comando';

  @override
  String get claudeDesktopAndOtherMcpClients =>
      'Claude Desktop e outros clientes MCP: adicione às configurações deles';

  @override
  String get settings => 'Configurações';

  @override
  String copy(Object title) {
    return 'Copiar: $title';
  }

  @override
  String isUsingTheMicrophoneRecordThis(Object app) {
    return '$app está usando o microfone. Gravar esta chamada?';
  }

  @override
  String get notNow => 'Agora não';

  @override
  String get record => 'Gravar';

  @override
  String get offerToRecordCalls => 'Oferecer gravar chamadas';

  @override
  String get whenTeamsZoomMeetInA =>
      'Quando o Teams, o Zoom, o Meet em um navegador ou outro app de chamadas começa a usar o microfone. Nada é gravado até você tocar em Gravar.';

  @override
  String get stopWhenTheCallEnds => 'Parar quando a chamada terminar';

  @override
  String get onlyForRecordingsStartedFromThe =>
      'Só para gravações iniciadas pela oferta, depois que o app de chamadas deixa o microfone por 20 segundos.';

  @override
  String get readSpeakerNamesFromTheCall =>
      'Ler os nomes de quem fala na janela da chamada';

  @override
  String get whileRecordingKoalinoReadsWhoTeams =>
      'Durante a gravação, o Koalino lê quem o Teams, o Zoom ou o Meet mostra falando, para sugerir nomes para vozes desconhecidas. Nada sai deste computador, e ninguém é nomeado até você tocar em Atribuir.';

  @override
  String get deleteThisChat => 'Excluir este chat?';

  @override
  String get itsQuestionsAndAnswersAreDeleted =>
      'As perguntas e respostas dele são apagadas deste dispositivo.';

  @override
  String get delete => 'Excluir';

  @override
  String get thatFileIsNoLongerIn => 'Esse arquivo não está mais no projeto.';

  @override
  String get whoAnswers => 'Quem responde';

  @override
  String get myPairedComputer => 'Meu computador pareado';

  @override
  String get thisPhoneSAiConnection => 'A conexão de IA deste telefone';

  @override
  String get deleteThisChat2 => 'Excluir este chat';

  @override
  String get askAboutYourNotesAnswersCome =>
      'Pergunte sobre suas notas. As respostas vêm do seu computador pareado, que lê a própria cópia das suas notas com a conexão escolhida lá para seus outros dispositivos.';

  @override
  String get askAboutYourNotesTheAnswer =>
      'Pergunte sobre suas notas. A resposta vem da sua conexão de IA padrão, que recebe as notas que couberem.';

  @override
  String get yourQuestion => 'Sua pergunta';

  @override
  String get ask => 'Perguntar';

  @override
  String get open => 'Abrir';

  @override
  String given(Object included) {
    return 'Fornecido: $included';
  }

  @override
  String leftOutForLength(Object omitted) {
    return 'Deixado de fora pelo tamanho: $omitted';
  }

  @override
  String answeredBy(Object connection) {
    return 'Respondido por $connection';
  }

  @override
  String get unableToOpenTheKeychain => 'Não foi possível abrir o chaveiro';

  @override
  String get koalinoKeepsTheKeyThatEncrypts =>
      'O Koalino guarda no chaveiro do sistema a chave que criptografa seus perfis de voz e a identidade do dispositivo. Desbloqueie o chaveiro, ou permita que o Koalino o use, e tente novamente. Se este dispositivo não tiver um serviço de chaveiro, instale ou inicie um e reinicie o Koalino.';

  @override
  String get unableToOpenYourNotes => 'Não foi possível abrir suas notas';

  @override
  String get koalinoCouldNotOpenItsDatabase =>
      'O Koalino não conseguiu abrir o banco de dados. Verifique se há espaço livre e tente novamente.';

  @override
  String get unlockingYourNotes => 'Desbloqueando suas notas…';

  @override
  String get quit => 'Sair';

  @override
  String get retry => 'Tentar novamente';

  @override
  String delete2(Object title) {
    return 'Excluir $title?';
  }

  @override
  String get itsTranscriptAttachmentsAndUnknownVoices =>
      'A transcrição, os anexos e as vozes desconhecidas dela são apagados para sempre, aqui e nos seus dispositivos pareados. As pessoas salvas permanecem.';

  @override
  String get unableToDeleteThisMeetingIf =>
      'Não foi possível excluir esta reunião. Se ela está sendo gravada, pare a gravação primeiro.';

  @override
  String get devices => 'Dispositivos';

  @override
  String get deviceSyncIsOff =>
      'A sincronização de dispositivos está desativada';

  @override
  String get meetingsStayOnThisDeviceUntil =>
      'As reuniões ficam neste dispositivo até a sincronização ser ativada.';

  @override
  String thisDevice3(Object capabilities) {
    return 'Este dispositivo\n$capabilities';
  }

  @override
  String get renameThisDevice => 'Renomear este dispositivo';

  @override
  String get discoverableYourOtherDevicesCanAsk =>
      'Visível: seus outros dispositivos podem pedir pareamento por alguns minutos.';

  @override
  String get makeDiscoverable => 'Tornar visível';

  @override
  String get pairingRequests => 'Pedidos de pareamento';

  @override
  String get acceptOnlyIfTheOtherDevice =>
      'Aceite só se o outro dispositivo mostrar o mesmo código.';

  @override
  String get reject => 'Recusar';

  @override
  String get accept => 'Aceitar';

  @override
  String get pairingInProgress => 'Pareamento em andamento';

  @override
  String get waitingForTheOtherDevice => 'Aguardando o outro dispositivo…';

  @override
  String get checkThatTheOtherDeviceShows =>
      'Verifique se o outro dispositivo mostra o mesmo código.';

  @override
  String get cancelPairing => 'Cancelar pareamento';

  @override
  String get codesMatch => 'Os códigos coincidem';

  @override
  String get pairedDevices => 'Dispositivos pareados';

  @override
  String get noPairedDevicesYet => 'Ainda não há dispositivos pareados.';

  @override
  String paired(Object pairedAt, Object hasRelay) {
    return 'Pareado em $pairedAt\n$hasRelay';
  }

  @override
  String remove3(Object deviceName) {
    return 'Remover $deviceName';
  }

  @override
  String get nearbyDevices => 'Dispositivos próximos';

  @override
  String get pair => 'Parear';

  @override
  String get anotherNetwork => 'Outra rede';

  @override
  String get showAnInviteHereAndUse =>
      'Mostre um convite aqui e use-o no outro dispositivo. Você ainda compara um código antes do pareamento.';

  @override
  String get showInvite => 'Mostrar convite';

  @override
  String get useInvite => 'Usar convite';

  @override
  String stopsSyncingWithThisDevicePair(Object deviceName) {
    return '$deviceName deixa de sincronizar com este dispositivo. Pareie de novo para retomar.';
  }

  @override
  String pairingCode(Object split) {
    return 'Código de pareamento $split';
  }

  @override
  String get noOtherDevicesFoundOnThis =>
      'Nenhum outro dispositivo encontrado nesta rede. Próximos só encontra dispositivos no mesmo Wi-Fi. Para um dispositivo em outra rede, use um convite.';

  @override
  String get noOtherDevicesFoundOnThis2 =>
      'Nenhum outro dispositivo encontrado nesta rede. Próximos só encontra dispositivos no mesmo Wi-Fi. Para um dispositivo em outra rede, use um convite ou defina um relay nas Configurações.';

  @override
  String get notConnectedItReconnectsWhenIt =>
      'Não conectado. Ele se reconecta quando estiver on-line e alcançar seu relay.';

  @override
  String get notConnectedPutBothDevicesOn =>
      'Não conectado. Coloque os dois dispositivos no mesmo Wi-Fi ou defina um relay nas Configurações.';

  @override
  String get whisperOnDevice => 'Whisper (no dispositivo)';

  @override
  String get openaiCloud => 'OpenAI (nuvem)';

  @override
  String gbGpu(Object round) {
    return 'GPU de $round GB';
  }

  @override
  String mbGpu(Object gpuMegabytes) {
    return 'GPU de $gpuMegabytes MB';
  }

  @override
  String get noTranscriptionModels => 'Nenhum modelo de transcrição';

  @override
  String get useYourOwnAiApp => 'Usar seu próprio app de IA';

  @override
  String get theMeetingItsNotesAndIts =>
      'A reunião, as notas e os arquivos dela vão para o app que você escolher, com sua própria conta. Para um resumo, copie a resposta lá e volte, ou compartilhe-a com o Koalino: o Koalino a guarda.';

  @override
  String summarizeIn(Object displayName) {
    return 'Resumir no $displayName';
  }

  @override
  String chatIn(Object displayName) {
    return 'Conversar no $displayName';
  }

  @override
  String get summarizeInAnotherApp => 'Resumir em outro app…';

  @override
  String get chatInAnotherApp => 'Conversar em outro app…';

  @override
  String summaryFromSavedTo(Object appName, Object title) {
    return 'Resumo do $appName salvo em \"$title\".';
  }

  @override
  String get undo => 'Desfazer';

  @override
  String get theEarlierSummaryIsBack => 'O resumo anterior foi restaurado.';

  @override
  String toKeepTheSummaryCopyS(Object appName) {
    return 'Para guardar o resumo, copie a resposta do $appName e volte ao Koalino, ou compartilhe a resposta com o Koalino.';
  }

  @override
  String get meetings => 'Reuniões';

  @override
  String get projects => 'Projetos';

  @override
  String get voices => 'Vozes';

  @override
  String get meeting2 => 'Reunião';

  @override
  String get chatAboutThisMeeting => 'Conversar sobre esta reunião';

  @override
  String chat(Object meeting) {
    return 'Chat: $meeting';
  }

  @override
  String get thisMeeting => 'esta reunião';

  @override
  String get takePhotos => 'Tirar fotos';

  @override
  String get attachFiles => 'Anexar arquivos';

  @override
  String get export => 'Exportar';

  @override
  String shareAs(Object label) {
    return 'Compartilhar como $label';
  }

  @override
  String get nameSpeakers => 'Nomear quem fala';

  @override
  String get selectAMeetingToSeeIts =>
      'Selecione uma reunião para ver a transcrição.';

  @override
  String get renameMeeting => 'Renomear reunião';

  @override
  String get deleteMeeting => 'Excluir reunião';

  @override
  String recordedOn(Object devices) {
    return 'Gravada em $devices';
  }

  @override
  String get split => 'Separar';

  @override
  String get longPressOrRightClickA =>
      'Toque e segure ou clique com o botão direito em um trecho para corrigi-lo.';

  @override
  String get noTranscriptIsAvailable => 'Nenhuma transcrição disponível.';

  @override
  String get theSpeakerIsAGuess => 'Quem fala é um palpite';

  @override
  String get peopleTalkedAtOnceHereWords =>
      'Várias pessoas falaram ao mesmo tempo aqui; podem faltar palavras';

  @override
  String get overlappingSpeech => 'Falas sobrepostas';

  @override
  String get someWordsWereUnclear => 'Algumas palavras não ficaram claras';

  @override
  String get addedAfterTheMeeting => 'Adicionado depois da reunião';

  @override
  String photoOpenFullScreen(Object fileName) {
    return 'Foto $fileName, abrir em tela cheia';
  }

  @override
  String get save => 'Salvar';

  @override
  String get preview => 'Pré-visualizar';

  @override
  String get editNotes => 'Editar notas';

  @override
  String get writeNotes => 'Escrever notas';

  @override
  String get loading => 'Carregando…';

  @override
  String get noNotesYet => 'Ainda não há notas.';

  @override
  String textAt(Object startTimeMs) {
    return 'Texto em $startTimeMs';
  }

  @override
  String get removedThisTranscriptWonTBe =>
      'Removido. Esta transcrição não será refeita depois da reunião.';

  @override
  String get savedThisTranscriptWonTBe =>
      'Salvo. Esta transcrição não será refeita depois da reunião.';

  @override
  String get editText => 'Editar texto';

  @override
  String get editTheTranscript => 'Editar a transcrição';

  @override
  String get leaveItEmptyToRemoveThis => 'Deixe vazio para remover esta parte.';

  @override
  String get searchMeetings => 'Pesquisar reuniões';

  @override
  String get chatWithAllYourNotes => 'Conversar com todas as suas notas';

  @override
  String get chatWithAllNotes => 'Conversar com todas as notas';

  @override
  String get refresh => 'Atualizar';

  @override
  String get noMeetingsYet => 'Ainda não há reuniões';

  @override
  String get recordingsYouStartAppearHere =>
      'As gravações que você iniciar aparecem aqui.';

  @override
  String get rename => 'Renomear';

  @override
  String get addToProject => 'Adicionar a um projeto…';

  @override
  String get summary2 => 'Resumo…';

  @override
  String get addToProject2 => 'Adicionar a um projeto';

  @override
  String writingTheSummaryOf(Object title) {
    return 'Escrevendo o resumo de $title';
  }

  @override
  String summaryOf(Object title) {
    return 'Resumo de $title';
  }

  @override
  String summarizeWith(Object label) {
    return 'Resumir com $label';
  }

  @override
  String defaultLabel2(Object model) {
    return '$model · padrão';
  }

  @override
  String get summarizeOnMyComputer => 'Resumir no meu computador';

  @override
  String get withClaudeCodeCodexOrThe =>
      'Com o Claude Code, o Codex ou a conexão configurada lá';

  @override
  String get useYourOwnAiApp2 => 'Usar seu próprio app de IA…';

  @override
  String get claudeChatgptOrAnotherAppWith =>
      'Claude, ChatGPT ou outro app, com sua conta';

  @override
  String get openSummary => 'Abrir resumo';

  @override
  String get editPromptForThisMeeting => 'Editar o prompt desta reunião…';

  @override
  String get useTheSettingsPrompt => 'Usar o prompt das Configurações';

  @override
  String get promptForThisMeeting => 'Prompt desta reunião';

  @override
  String placeholders(Object placeholders) {
    return 'Variáveis: $placeholders';
  }

  @override
  String get addAnAiConnectionInSettings3 =>
      'Adicione uma conexão de IA nas Configurações para escrever resumos.';

  @override
  String get summarizeWith2 => 'Resumir com';

  @override
  String get summarize => 'Resumir';

  @override
  String get summarizeAgain => 'Resumir de novo';

  @override
  String get editPromptForThisMeeting2 => 'Editar o prompt desta reunião';

  @override
  String get writingTheSummaryALocalModel =>
      'Escrevendo o resumo… Um modelo local pode levar alguns minutos.';

  @override
  String get noSummaryYet => 'Ainda não há resumo.';

  @override
  String get writeASummary => 'Escrever um resumo';

  @override
  String writtenByYouOn(Object createdAt) {
    return 'Escrito por você em $createdAt';
  }

  @override
  String writtenByOn(
      Object model, Object connectionLabel, Object createdAt, Object edited) {
    return 'Escrito por $model ($connectionLabel) em $createdAt$edited';
  }

  @override
  String get editSummary => 'Editar resumo';

  @override
  String get tryAgainOnMyComputer => 'Tentar de novo no meu computador';

  @override
  String get cancelRequest => 'Cancelar solicitação';

  @override
  String get notesFolder => 'Pasta de notas';

  @override
  String get keepYourProjectsAndMeetingsAs =>
      'Mantenha seus projetos e reuniões como arquivos simples em uma pasta, para lê-los e editá-los com qualquer editor. As edições em notes.md, summary.md e nos arquivos de projeto voltam ao app em meio minuto, e os arquivos que você colocar na pasta de um projeto ou na pasta de anexos de uma reunião são adicionados. O app reescreve o transcript.md: corrija a transcrição no app.';

  @override
  String get theFolderIsADecryptedCopy =>
      'A pasta é uma cópia descriptografada das suas notas. Quem puder lê-la poderá lê-las, e ferramentas de backup ou sincronização vão copiá-la. Mantenha-a em um disco criptografado, dentro da sua própria pasta de usuário.';

  @override
  String get folder => 'Pasta';

  @override
  String get chooseAFolder => 'Escolher uma pasta';

  @override
  String get keepTheNotesFolder => 'Manter a pasta de notas';

  @override
  String get syncNow => 'Sincronizar agora';

  @override
  String lastSyncAtFailed(Object at, Object code) {
    return 'A última sincronização às $at falhou: $code';
  }

  @override
  String lastSyncAtFilesWrittenEdits(
      Object at, Object written, Object imported) {
    return 'Última sincronização às $at: $written arquivos gravados, $imported edições e arquivos incorporados.';
  }

  @override
  String get inviteADevice => 'Convidar um dispositivo';

  @override
  String get pairingInviteQrCode => 'Código QR do convite de pareamento';

  @override
  String onTheOtherDeviceOpenDevices(Object hour, Object minute) {
    return 'No outro dispositivo, abra Dispositivos, escolha Usar convite e cole este link. Você pode escanear o código com um app de câmera para copiá-lo. Ele funciona até $hour:$minute.';
  }

  @override
  String get close => 'Fechar';

  @override
  String get inviteLinkCopied => 'Link do convite copiado';

  @override
  String get copyLink => 'Copiar link';

  @override
  String get deviceFoundChoosePairUnderNearby =>
      'Dispositivo encontrado. Escolha Parear em Dispositivos próximos.';

  @override
  String get useAnInvite => 'Usar um convite';

  @override
  String get inviteLink => 'Link do convite';

  @override
  String get findDevice => 'Encontrar dispositivo';

  @override
  String get pairWithAnotherDevice => 'Parear com outro dispositivo?';

  @override
  String get youOpenedAPairingInviteKoalino =>
      'Você abriu um convite de pareamento. O Koalino vai encontrar o dispositivo que o mostra, e os dois dispositivos vão mostrar um código para comparar antes de qualquer compartilhamento.';

  @override
  String get pairingStartedCheckThatBothDevices =>
      'Pareamento iniciado. Verifique se os dois dispositivos mostram o mesmo código.';

  @override
  String get done => 'Concluído';

  @override
  String get photosAreAddedToTheMeeting =>
      'As fotos são adicionadas à reunião conforme você as tira.';

  @override
  String get takePhoto => 'Tirar foto';

  @override
  String get theCameraIsUnavailable => 'A câmera não está disponível.';

  @override
  String search(Object name) {
    return 'Pesquisar em $name';
  }

  @override
  String get noteSaved => 'Nota salva';

  @override
  String get theNoteCouldNotBeSaved => 'Não foi possível salvar a nota.';

  @override
  String couldNotBeAdded2(Object failed) {
    return '$failed não puderam ser adicionadas.';
  }

  @override
  String get renameProject => 'Renomear projeto';

  @override
  String get theProjectCouldNotBeRenamed =>
      'Não foi possível renomear o projeto.';

  @override
  String delete3(Object project) {
    return 'Excluir \"$project\"?';
  }

  @override
  String get itsFilesNotesAndEMails =>
      'Os arquivos, notas e e-mails dele são apagados em todos os dispositivos pareados. As reuniões dele são mantidas, fora de qualquer projeto.';

  @override
  String get theProjectCouldNotBeDeleted =>
      'Não foi possível excluir o projeto.';

  @override
  String get itIsDeletedOnEveryPaired =>
      'Ele é apagado em todos os dispositivos pareados.';

  @override
  String get theFileCouldNotBeDeleted => 'Não foi possível excluir o arquivo.';

  @override
  String get theMeetingCouldNotBeRemoved =>
      'Não foi possível tirar a reunião do projeto.';

  @override
  String get project => 'Projeto';

  @override
  String get searchThisProject => 'Pesquisar neste projeto';

  @override
  String get chatAboutThisProject => 'Conversar sobre este projeto';

  @override
  String get addFiles => 'Adicionar arquivos';

  @override
  String get writeANote => 'Escrever uma nota';

  @override
  String get addMeetings => 'Adicionar reuniões';

  @override
  String get deleteProject => 'Excluir projeto';

  @override
  String get addThemHereOrFromA =>
      'Adicione-as aqui ou pela página de uma reunião.';

  @override
  String get removeFromProject => 'Tirar do projeto';

  @override
  String get filesNotesAndEMails => 'Arquivos, notas e e-mails';

  @override
  String get nothingAddedYet => 'Nada adicionado ainda';

  @override
  String get addPicturesMarkdownPdfsEMails =>
      'Adicione imagens, Markdown, PDFs, e-mails (.eml) ou documentos.';

  @override
  String get onItsWayFromAPaired => 'A caminho de um dispositivo pareado';

  @override
  String get newNote => 'Nova nota';

  @override
  String get noteMarkdown => 'Nota (Markdown)';

  @override
  String get everyMeetingIsAlreadyInA =>
      'Todas as reuniões já estão em um projeto.';

  @override
  String get add => 'Adicionar';

  @override
  String get moveToProject => 'Mover para o projeto';

  @override
  String get noProject => 'Sem projeto';

  @override
  String get newProject => 'Novo projeto…';

  @override
  String get newProject2 => 'Novo projeto';

  @override
  String get create => 'Criar';

  @override
  String get theProjectCouldNotBeCreated => 'Não foi possível criar o projeto.';

  @override
  String get theMeetingCouldNotBeMoved => 'Não foi possível mover a reunião.';

  @override
  String get removedFromItsProject => 'Removida do projeto';

  @override
  String movedTo(Object project) {
    return 'Movida para $project';
  }

  @override
  String get searchProjects => 'Pesquisar projetos';

  @override
  String get noProjectsYetAProjectGathers =>
      'Ainda não há projetos. Um projeto reúne reuniões com os arquivos, notas e e-mails que pertencem a elas.';

  @override
  String get theRecordingIsStillStartingTry =>
      'A gravação ainda está iniciando; tente novamente.';

  @override
  String recording(Object title) {
    return 'Gravando \"$title\"';
  }

  @override
  String get recording2 => 'Gravando';

  @override
  String paused(Object title) {
    return '\"$title\" pausada';
  }

  @override
  String get paused2 => 'Pausada';

  @override
  String get finishingTranscription => 'Finalizando a transcrição…';

  @override
  String get meetingTitle => 'Título da reunião';

  @override
  String get enterMeetingTitleOptional =>
      'Digite o título da reunião (opcional)';

  @override
  String get leaveEmptyToNameTheMeeting =>
      'Deixe vazio para nomear a reunião pelo horário de início';

  @override
  String get startingRecording => 'Iniciando a gravação';

  @override
  String get startRecording => 'Iniciar gravação';

  @override
  String get pause => 'Pausar';

  @override
  String get resume => 'Retomar';

  @override
  String get stop => 'Parar';

  @override
  String get language => 'Idioma';

  @override
  String get aChangeAppliesToWhatIs =>
      'Uma mudança vale para o que for dito a seguir';

  @override
  String get liveTranscript => 'Transcrição ao vivo';

  @override
  String get theTranscriptAppearsHereAFew =>
      'A transcrição aparece aqui alguns segundos depois de cada pausa na fala.';

  @override
  String get nearbyDevicesSeeThisNameWhile =>
      'Os dispositivos próximos veem este nome enquanto este dispositivo está visível. Os dispositivos já pareados mantêm o nome que conheceram.';

  @override
  String get meetingRenamed => 'Reunião renomeada';

  @override
  String get thisTitleIsnTAllowed => 'Este título não é permitido.';

  @override
  String get couldnTSaveTheNewTitle =>
      'Não foi possível salvar o novo título. Tente novamente.';

  @override
  String get theNoteTakingEngineIsnT =>
      'O mecanismo de anotações não está disponível agora.';

  @override
  String get thisMeetingNoLongerExists2 => 'Esta reunião não existe mais.';

  @override
  String get couldnTRenameTheMeeting => 'Não foi possível renomear a reunião.';

  @override
  String get thisFileIsStillOnIts =>
      'Este arquivo ainda está a caminho de um dispositivo pareado.';

  @override
  String get thisPictureCannotBeShown => 'Não é possível mostrar esta imagem.';

  @override
  String get thisKindOfDocumentCanT =>
      'Este tipo de documento ainda não pode ser mostrado no app.';

  @override
  String get thisFileCannotBeOpened => 'Não é possível abrir este arquivo.';

  @override
  String get thisFileCannotBeRead => 'Não é possível ler este arquivo.';

  @override
  String get thisEMailCannotBeRead => 'Não é possível ler este e-mail.';

  @override
  String get noSubject => '(sem assunto)';

  @override
  String get from => 'De';

  @override
  String get to => 'Para';

  @override
  String get cc => 'Cc';

  @override
  String get sent => 'Enviado';

  @override
  String attachments(Object attachmentNames) {
    return 'Anexos: $attachmentNames';
  }

  @override
  String get theRestOfThisEMail =>
      'O restante deste e-mail é longo demais para mostrar.';

  @override
  String get searching => 'Pesquisando…';

  @override
  String get nothingFound => 'Nada encontrado.';

  @override
  String get search2 => 'Pesquisar';

  @override
  String get pairYourDevicesAndSeeWhich =>
      'Pareie seus dispositivos e veja quais estão conectados.';

  @override
  String get transcriptionModel => 'Modelo de transcrição';

  @override
  String get chooseTheWhisperModel => 'Escolha o modelo do Whisper.';

  @override
  String get languageModelsForMeetingSummariesAnd =>
      'Modelos de linguagem para resumos de reuniões e o chat, e os seus prompts.';

  @override
  String get keepProjectsAndMeetingsAsFiles =>
      'Mantenha projetos e reuniões como arquivos que você pode editar com qualquer editor.';

  @override
  String get letYourOwnAssistantSuchAs =>
      'Deixe seu próprio assistente, como o Claude Code, ler suas notas.';

  @override
  String get appLock => 'Bloqueio do app';

  @override
  String get defaultLanguage => 'Idioma padrão';

  @override
  String get usedForNewRecordingsAutoDetects =>
      'Usado em novas gravações. Auto detecta o idioma conforme as pessoas falam.';

  @override
  String get transcribeOn => 'Transcrever em';

  @override
  String get anotherDeviceReceivesTheRecordingS =>
      'Outro dispositivo recebe o áudio da gravação, criptografado, e o apaga depois de transcrever. Se ele não estiver disponível, este dispositivo transcreve.';

  @override
  String get telemetry => 'Telemetria';

  @override
  String get howLongTranscriptionTakesOnEach =>
      'Quanto tempo a transcrição leva em cada dispositivo e o consumo de energia estimado.';

  @override
  String get logLevel => 'Nível de log';

  @override
  String get howMuchDetailTheAppWrites =>
      'Quanto detalhe o app escreve nos logs. Depuração e Rastreamento ajudam a diagnosticar problemas, mas escrevem mais.';

  @override
  String get custom => 'Personalizado';

  @override
  String get shareLogs => 'Compartilhar logs';

  @override
  String get theLast7DaysOfDiagnostic =>
      'Os últimos 7 dias de logs de diagnóstico, por exemplo para anexar a um relatório de bug.';

  @override
  String get unableToReadTheAppLock =>
      'Não foi possível ler a configuração do bloqueio do app.';

  @override
  String get needsAndroid10OrLater => 'Requer Android 10 ou posterior.';

  @override
  String get setAScreenLockInAndroid =>
      'Configure um bloqueio de tela nas configurações do Android para usar o bloqueio do app.';

  @override
  String askForYourFingerprintFaceOr(Object lockAfterBackground) {
    return 'Pedir sua digital, seu rosto ou o bloqueio de tela quando você voltar ao Koalino depois de $lockAfterBackground segundos fora. A gravação e a sincronização continuam enquanto ele está bloqueado.';
  }

  @override
  String get otherNetworks => 'Outras redes';

  @override
  String get aRelayOnYourOwnAlways =>
      'Um relay em um dispositivo seu sempre ligado permite que seus dispositivos se alcancem longe do mesmo Wi-Fi. A sincronização na mesma rede continua funcionando sem ele.';

  @override
  String get relayAddress => 'Endereço do relay';

  @override
  String get deviceAddressesOptionalOnePerLine =>
      'Endereços de dispositivos (opcional, um por linha)';

  @override
  String get saveAddresses => 'Salvar endereços';

  @override
  String get nothingThatWasSharedCouldBe =>
      'Nada do que foi compartilhado pôde ser lido.';

  @override
  String couldNotBeRead(Object message, Object failedCount) {
    return '$message; $failedCount não puderam ser lidos';
  }

  @override
  String saveAsTheSummaryOf(Object title) {
    return 'Salvar como resumo de \"$title\"';
  }

  @override
  String get addToAProjectInstead => 'Adicionar a um projeto em vez disso';

  @override
  String get splitThisMeeting => 'Separar esta reunião?';

  @override
  String itWasRecordedOnAndThe(Object devices, Object recordings) {
    return 'Ela foi gravada em $devices, e o app juntou as gravações. Separar faz a outra gravação ($recordings) voltar a aparecer sozinha, com a própria transcrição, aqui e nos seus dispositivos pareados. Elas não serão juntadas de novo. Esta reunião mantém a transcrição.';
  }

  @override
  String get unableToSplitThisMeeting =>
      'Não foi possível separar esta reunião.';

  @override
  String get telemetryDashboard => 'Painel de telemetria';

  @override
  String get transcription => 'Transcrição';

  @override
  String get latency => 'Latência';

  @override
  String get tasks => 'Tarefas';

  @override
  String get power => 'Energia';

  @override
  String get transcriptionDurationOverTime =>
      'Duração da transcrição ao longo do tempo';

  @override
  String get latencyDistribution => 'Distribuição da latência';

  @override
  String get taskTypeBreakdown => 'Divisão por tipo de tarefa';

  @override
  String get powerConsumptionEstimate => 'Estimativa de consumo de energia';

  @override
  String get fastestLeastAccurate => 'O mais rápido, o menos preciso';

  @override
  String get fastManyMistakesTheAppShips =>
      'Rápido, muitos erros. O app vem com este';

  @override
  String get fastManyMistakes => 'Rápido, muitos erros';

  @override
  String get goodBalanceOnMostComputers =>
      'Bom equilíbrio na maioria dos computadores';

  @override
  String get accurateSlowerNeedsAbout2Gb =>
      'Preciso, mais lento, precisa de cerca de 2 GB de memória';

  @override
  String get olderNeedsAPowerfulComputer =>
      'Mais antigo, precisa de um computador potente';

  @override
  String get mostAccurateNeedsAPowerfulComputer =>
      'O mais preciso, precisa de um computador potente';

  @override
  String get veryAccurateBestWithAGraphics =>
      'Muito preciso. Melhor com uma placa de vídeo';

  @override
  String get thisModelMayBeTooLarge => 'Este modelo pode ser grande demais';

  @override
  String mayNeedMoreMemoryThanThis(Object model) {
    return '$model pode precisar de mais memória do que este dispositivo tem, e as gravações podem falhar ou ficar lentas.';
  }

  @override
  String get downloadAnyway => 'Baixar mesmo assim';

  @override
  String get transcriptionSettingsAreUnavailable =>
      'As configurações de transcrição não estão disponíveis.';

  @override
  String get downloadFromQualcomm => 'Baixar da Qualcomm?';

  @override
  String get about360MbIsDownloadedFrom =>
      'Cerca de 360 MB são baixados da Qualcomm e ocupam cerca de 460 MB:\n\n• O runtime QNN da Qualcomm, que permite aos apps usar a NPU. Ele é licenciado pela AI Stack License da Qualcomm, não pela licença do app.\n• Whisper Small, compilado pelo Qualcomm AI Hub para a NPU deste telefone.\n\nCada arquivo é verificado antes de ser usado. Suas gravações nunca saem do telefone. Ao baixar, você aceita os termos da Qualcomm.';

  @override
  String get acceptAndDownload => 'Aceitar e baixar';

  @override
  String get downloadingTheNpuFiles => 'Baixando os arquivos da NPU…';

  @override
  String get transcribeOnTheNpu => 'Transcrever na NPU';

  @override
  String get thisPhoneHasAnAiChip =>
      'Este telefone tem um chip de IA (NPU) que pode executar o Whisper Small deixando o processador livre: 30 segundos de fala levam cerca de 2 segundos. É preciso baixar arquivos da Qualcomm.';

  @override
  String get download => 'Baixar';

  @override
  String get runsWhisperSmallOnThePhone =>
      'Executa o Whisper Small no chip de IA do telefone, no lugar do modelo escolhido abaixo, inclusive na passagem depois da reunião, a menos que ela tenha um modelo próprio. Os tempos das palavras são aproximados.';

  @override
  String get removeTheNpuFiles => 'Remover os arquivos da NPU';

  @override
  String get alsoRecordTheComputerSSound => 'Gravar também o som do computador';

  @override
  String get transcribesWhatTheComputerPlaysSuch =>
      'Transcreve o que o computador reproduz, como o outro lado de uma chamada, junto com o microfone. Quando o microfone também capta os alto-falantes, as palavras repetidas são mantidas uma vez só.';

  @override
  String get improveTheTranscriptAfterTheMeeting =>
      'Melhorar a transcrição depois da reunião';

  @override
  String get transcribesEachMeetingAgainWithMore =>
      'Transcreve cada reunião de novo com mais contexto quando ela termina; um telefone espera estar carregando. Até lá a fala fica guardada criptografada, depois é apagada.';

  @override
  String get modelForThisPass => 'Modelo para esta passagem';

  @override
  String get aLargerModelIsSlowerBut =>
      'Um modelo maior é mais lento, porém mais preciso, e há tempo depois da reunião. Baixe modelos acima.';

  @override
  String get sameAsForRecording => 'O mesmo da gravação';

  @override
  String get keepSpeechForCombining => 'Guardar a fala para combinar';

  @override
  String get whenAMeetingWasAlsoRecorded =>
      'Quando uma reunião também foi gravada em outro dispositivo, a fala dela espera esse tempo para ser combinada, depois é apagada.';

  @override
  String get whisperModel => 'Modelo do Whisper';

  @override
  String get biggerModelsMakeFewerMistakesAnd =>
      'Modelos maiores erram menos, mas precisam de mais tempo e memória. Os downloads vêm do Hugging Face. Esta escolha vale só para este dispositivo.';

  @override
  String automatic2(Object fileName, Object mode) {
    return 'Automático: $fileName$mode';
  }

  @override
  String usesUntilIsDownloadedTheBest(
      Object fileName, Object waiting, Object reason) {
    return 'Usa $fileName até $waiting ser baixado, o mais adequado para este dispositivo. $reason';
  }

  @override
  String picksTheBestModelForThis(Object fileName, Object mode) {
    return 'Escolhe o melhor modelo para este dispositivo: $fileName$mode.';
  }

  @override
  String download2(Object sizeBytes) {
    return 'Baixar $sizeBytes';
  }

  @override
  String tryLabel(Object model) {
    return 'Experimentar $model';
  }

  @override
  String get use => 'Usar';

  @override
  String suggestedForThisDevice(Object model, Object mode) {
    return 'Sugerido para este dispositivo: $model$mode';
  }

  @override
  String get efficientMode => 'Modo eficiente';

  @override
  String runsTheSameModelsOnThe(Object gpuName) {
    return 'Executa os mesmos modelos no chip gráfico ($gpuName), geralmente mais rápido. Desativado usa o processador, como de costume.';
  }

  @override
  String get theGraphicsChipStoppedTheApp =>
      'Da última vez o chip gráfico parou o app, então este dispositivo está transcrevendo com o processador.';

  @override
  String get tryTheGraphicsChipAgain => 'Tentar o chip gráfico de novo';

  @override
  String get onThisDevice => 'neste dispositivo';

  @override
  String get usedByAutomatic => 'usado pelo Automático';

  @override
  String get englishOnly => 'Só inglês';

  @override
  String get mayBeTooLargeForThis =>
      'pode ser grande demais para este dispositivo';

  @override
  String get doesnTMatchYourLanguage => 'não corresponde ao seu idioma';

  @override
  String get notEnoughFreeSpace => 'espaço livre insuficiente';

  @override
  String get cancelDownload => 'Cancelar download';

  @override
  String get glossary => 'Glossário';

  @override
  String get namesProjectNamesAndTermsOne =>
      'Nomes, nomes de projetos e termos, um por linha. A transcrição os escreve como aqui.';

  @override
  String get saveGlossary => 'Salvar glossário';

  @override
  String get voiceGallery => 'Galeria de vozes';

  @override
  String get tryAgain => 'Tentar novamente';

  @override
  String get unknownVoices => 'Vozes desconhecidas';

  @override
  String get noUnknownSpeakersToReview =>
      'Nenhuma voz desconhecida para revisar';

  @override
  String get savedPeople => 'Pessoas salvas';

  @override
  String get noSavedPeopleYetNameA =>
      'Ainda não há pessoas salvas. Dê um nome a uma voz para adicionar alguém.';

  @override
  String delete4(Object displayName) {
    return 'Excluir $displayName';
  }

  @override
  String get playing => 'Reproduzindo…';

  @override
  String get untitledMeeting => 'Reunião sem título';

  @override
  String get play => 'Reproduzir';

  @override
  String get deleteThisVoice => 'Excluir esta voz';

  @override
  String theCallShowedSpeakingCheckThen(Object suggestedName) {
    return 'A chamada mostrava $suggestedName falando. Confira e depois toque em Atribuir.';
  }

  @override
  String get chooseASavedPerson => 'Escolha uma pessoa salva';

  @override
  String get assign => 'Atribuir';

  @override
  String get deleteTheseVoices => 'Excluir estas vozes';

  @override
  String get theseSoundLikeOnePersonUntick =>
      'Parecem ser uma só pessoa. Desmarque as que forem de outra pessoa.';

  @override
  String itsVoiceSamplesGoForGood(Object voices) {
    return 'As $voices amostras de voz são apagadas para sempre, aqui e nos seus dispositivos pareados. As transcrições permanecem.';
  }

  @override
  String get itsVoiceSampleGoesForGood =>
      'A amostra de voz é apagada para sempre, aqui e nos seus dispositivos pareados. A transcrição permanece.';

  @override
  String get theirNameAndVoiceSamplesGo =>
      'O nome e as amostras de voz são apagados para sempre, aqui e nos seus dispositivos pareados. As transcrições permanecem e numeram a pessoa como participante.';

  @override
  String unknownSpeaker(Object index) {
    return 'Voz desconhecida $index';
  }

  @override
  String attachedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count arquivos anexados',
      one: '1 arquivo anexado',
    );
    return '$_temp0';
  }

  @override
  String addedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count arquivos adicionados',
      one: '1 arquivo adicionado',
    );
    return '$_temp0';
  }

  @override
  String emailAttachmentsLeft(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count anexos ficaram no e-mail',
      one: '1 anexo ficou no e-mail',
    );
    return '$_temp0';
  }

  @override
  String addedMeetings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count reuniões adicionadas',
      one: '1 reunião adicionada',
    );
    return '$_temp0';
  }

  @override
  String otherRecordings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count gravações',
      one: '1 gravação',
    );
    return '$_temp0';
  }

  @override
  String get severalDevices => 'vários dispositivos';

  @override
  String meetingCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count reuniões',
      one: '1 reunião',
    );
    return '$_temp0';
  }

  @override
  String fileCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count arquivos',
      one: '1 arquivo',
    );
    return '$_temp0';
  }

  @override
  String chunkCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count trechos',
      one: '1 trecho',
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
      ', há mais não mostrados: adicione palavras para restringir';

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
      other: '$count dias',
      one: '1 dia',
    );
    return '$_temp0';
  }

  @override
  String voiceSampleCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count amostras de voz',
      one: '1 amostra de voz',
      zero: 'ainda sem amostra de voz, por isso não é reconhecido pela voz',
    );
    return '$_temp0';
  }

  @override
  String picturesIncluded(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count imagens foram incluídas',
      one: '1 imagem foi incluída',
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
    return '$parts em $meetings';
  }

  @override
  String assignedVoicesTo(int count, Object name) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count vozes atribuídas a $name',
      one: 'Atribuído a $name',
    );
    return '$_temp0';
  }

  @override
  String addSharedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Adicionar $count arquivos compartilhados a um projeto',
      one: 'Adicionar o arquivo compartilhado a um projeto',
    );
    return '$_temp0';
  }

  @override
  String keepSpeechSaved(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Salvo. Agora a fala espera até $count dias para ser combinada.',
      one: 'Salvo. Agora a fala espera até 1 dia para ser combinada.',
    );
    return '$_temp0';
  }

  @override
  String titlesCanBeAtMost(Object max) {
    return 'Os títulos podem ter no máximo $max caracteres.';
  }

  @override
  String promptUnknownPlaceholder(Object example) {
    return 'O prompt tem uma variável $example desconhecida ou não fechada.';
  }

  @override
  String get live => '● Ao vivo';

  @override
  String get offline => '○ Off-line';

  @override
  String minutesShort(Object minutes) {
    return '$minutes min';
  }

  @override
  String get appLanguage => 'Idioma do app';

  @override
  String get appLanguageDescription =>
      'O idioma dos menus e mensagens do Koalino.';

  @override
  String get systemLanguage => 'Padrão do sistema';

  @override
  String photosAddedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count fotos adicionadas',
      one: '1 foto adicionada',
    );
    return '$_temp0';
  }

  @override
  String moreModels(int count) {
    return 'Mais modelos ($count)';
  }

  @override
  String get moreModelsHint =>
      'Versões mais leves que erram um pouco mais, versões só em inglês e modelos que talvez não caibam neste dispositivo.';

  @override
  String get microphoneSilencedByCall =>
      'O Android está silenciando o microfone enquanto outro app o usa para uma chamada: nada é gravado até a chamada terminar.';

  @override
  String get handoffInTheBrowser =>
      'O Koalino abre o site no seu navegador, onde tem sessão iniciada com a sua própria conta, e passa-lhe a reunião com o seu prompt. A janela seguinte mostra o que fazer lá.';

  @override
  String toKeepTheSummaryCopyTheWebsite(String appName) {
    return 'Para guardar o resumo, copie a resposta do $appName no site e volte ao Koalino.';
  }

  @override
  String get useYourClaudeOrChatgptPlan =>
      'Use o seu plano do Claude ou do ChatGPT';

  @override
  String get summariesCanUseYourOwnPlan =>
      'Os resumos podem usar a sua própria assinatura do Claude ou do ChatGPT, sem chave de API. Instale o Claude Code ou o Codex e entre uma vez: depois o Koalino escreve os resumos sozinho.';

  @override
  String get installClaudeCode => 'Instalar o Claude Code';

  @override
  String get forAClaudePlanOpensTheGuide =>
      'Para um plano do Claude. Abre o guia de instalação.';

  @override
  String get installCodex => 'Instalar o Codex';

  @override
  String get forAChatgptPlanOpensTheGuide =>
      'Para um plano do ChatGPT. Abre o guia de instalação.';

  @override
  String get skipUseTheWebsite => 'Pular: usar o site';

  @override
  String get koalinoCopiesTheMeetingAndOpens =>
      'Nada para instalar: o Koalino copia a reunião e abre claude.ai ou chatgpt.com, e você cola lá.';

  @override
  String get addAConnection => 'Adicionar uma conexão';

  @override
  String get claudeCodeOrCodexAlreadyInstalled =>
      'Claude Code ou Codex já instalado, ou um servidor na sua rede.';

  @override
  String get noConnectionNeededUseTheWebsite =>
      'Não é preciso conexão. Numa reunião, abra Resumo e escolha Usar seu próprio app de IA…: o Koalino copia a reunião e abre claude.ai ou chatgpt.com.';

  @override
  String get afterInstallingAddTheConnection =>
      'Depois de instalar e entrar, volte e escolha Adicionar conexão → Adicionar uma conexão → Claude Code ou Codex.';

  @override
  String get claudeAiOrChatgptComInYourBrowser =>
      'claude.ai ou chatgpt.com no navegador, com a sua conta';

  @override
  String get newMeeting => 'Nova reunião';

  @override
  String get newMeetingExplanation =>
      'Uma reunião sem gravação. Escreva notas ou adicione ficheiros: os resumos e o chat usam-nos.';

  @override
  String get titleOptional => 'Título (opcional)';

  @override
  String handoffStepPressEnter(String site) {
    return '$site abre com a reunião já na caixa de mensagem. Prima Enter para a enviar.';
  }

  @override
  String handoffStepSentByItself(String site) {
    return '$site abre e envia a reunião sozinho. Inicie sessão primeiro, se pedir.';
  }

  @override
  String handoffStepPaste(String site) {
    return '$site abre e o Koalino copia a reunião. Clique na caixa de mensagem, cole-a (Ctrl+V) e prima Enter.';
  }

  @override
  String get handoffStepDragFiles =>
      'Abre-se também uma pasta com os ficheiros: arraste-os todos para a caixa da mensagem, para que sigam como ficheiros.';

  @override
  String get handoffStepCopyAnswer =>
      'Quando a resposta estiver completa, prima o botão de copiar por baixo dela.';

  @override
  String get handoffStepComeBack =>
      'Volte ao Koalino: a resposta passa a ser o resumo da reunião, com Anular.';

  @override
  String get handoffStepAskThere =>
      'Depois, faça lá as suas perguntas sobre a reunião.';

  @override
  String openSite(String site) {
    return 'Abrir $site';
  }

  @override
  String writtenBySourceOn(String source, String createdAt) {
    return 'Escrito por $source em $createdAt';
  }

  @override
  String get noRecordingWriteNotesOrAttach =>
      'Sem gravação. Escreva notas ou anexe ficheiros: os resumos e o chat usam-nos.';

  @override
  String handoffStepOpensFilled(String site) {
    return '$site abre com a reunião já na caixa de mensagem.';
  }

  @override
  String handoffStepOpensCopied(String site) {
    return '$site abre e o Koalino copia a reunião.';
  }

  @override
  String get handoffStepSend => 'Prima Enter para a enviar.';

  @override
  String get handoffStepPasteSend =>
      'Clique na caixa de mensagem, cole a reunião (Ctrl+V) e prima Enter.';

  @override
  String get chatHandoffIntro =>
      'Faça as suas perguntas na sua própria app de IA, com a sua própria conta. O Koalino passa-lhe as notas e as transcrições, e as fotos e os ficheiros vão junto como ficheiros.';

  @override
  String get plainTextFile => 'Texto simples (.txt)';

  @override
  String get unableToExportTheSummary => 'Não foi possível exportar o resumo';

  @override
  String get addAVoice => 'Adicionar uma voz';

  @override
  String alreadySavedVoiceAddsToThem(String name) {
    return '$name já está salvo: esta voz será adicionada a essa pessoa.';
  }

  @override
  String addVoicesTo(String name) {
    return 'Adicionar vozes a $name';
  }

  @override
  String tickTheVoicesThatAre(String name) {
    return 'Estas vozes não foram reconhecidas como de ninguém. Marque as de $name; use Reproduzir para conferir.';
  }

  @override
  String addTo(String name) {
    return 'Adicionar a $name';
  }

  @override
  String get apiKey => 'Chave de API';

  @override
  String get onlyForOnlineServicesKeptIn =>
      'Só para serviços online. Guardada no chaveiro deste dispositivo e enviada só para este endereço.';

  @override
  String get aKeyIsSavedLeaveEmpty =>
      'Há uma chave salva. Deixe vazio para mantê-la.';

  @override
  String get removeTheSavedKey => 'Remover a chave salva';

  @override
  String get presetOllama =>
      'No seu computador ou rede: grátis, privado, funciona offline (Gemma, Qwen, Llama…).';

  @override
  String get presetLmStudio =>
      'Como o Ollama, com um app de desktop para escolher modelos.';

  @override
  String get presetGemini =>
      'Google. Nível gratuito com limites; nele o Google pode usar o que você envia para melhorar seus produtos. Chave: aistudio.google.com.';

  @override
  String get presetDeepseek =>
      'Pago por uso, muito barato, crédito pré-pago. Chave: platform.deepseek.com.';

  @override
  String get presetQwen =>
      'Alibaba. Pago por uso. Chave: Alibaba Cloud Model Studio.';

  @override
  String get presetKimi =>
      'Moonshot AI. Pago por uso. Chave: platform.moonshot.ai.';

  @override
  String get presetMistral =>
      'Mistral (França). Nível gratuito com limites rígidos. Chave: console.mistral.ai.';

  @override
  String get presetOpenrouter =>
      'Uma chave para muitos modelos (Gemini, DeepSeek, Qwen…), alguns grátis; o texto também passa pelo OpenRouter. Chave: openrouter.ai.';

  @override
  String get keepMeetingRecordings => 'Guardar as gravações das reuniões';

  @override
  String get keepMeetingRecordingsSubtitle =>
      'Guarda o áudio de cada reunião depois da transcrição, criptografado neste dispositivo, para ouvir de novo. Desativado por padrão.';

  @override
  String get keepMeetingRecordingsQuestion =>
      'Guardar as gravações das reuniões?';

  @override
  String get keepRecordingsWarning =>
      'As gravações contêm as vozes de todos na reunião. Avise as pessoas que você grava e siga as regras de consentimento do seu país: em muitos lugares gravar outras pessoas exige permissão. As gravações ficam criptografadas neste dispositivo e não são sincronizadas; ocupam cerca de 2 MB a cada 10 minutos de fala. São apagadas com a reunião, ou todas de uma vez com Apagar todas as gravações. Só a fala é guardada, então os trechos em silêncio tocam como silêncio.';

  @override
  String get keepRecordings => 'Guardar gravações';

  @override
  String recordingsKept(int count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count gravações guardadas · $size MB',
      one: '1 gravação guardada · $size MB',
      zero: 'Nenhuma gravação guardada',
    );
    return '$_temp0';
  }

  @override
  String get wipeAllRecordings => 'Apagar todas as gravações';

  @override
  String get wipeAllRecordingsQuestion => 'Apagar todas as gravações?';

  @override
  String get wipeAllRecordingsExplanation =>
      'Todas as gravações guardadas neste dispositivo são apagadas. Transcrições e notas ficam. Não é possível desfazer.';

  @override
  String recordingsWiped(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count gravações apagadas.',
      one: '1 gravação apagada.',
      zero: 'Nenhuma gravação para apagar.',
    );
    return '$_temp0';
  }

  @override
  String get recordingsUnavailable =>
      'As gravações não estão acessíveis agora.';

  @override
  String get playRecording => 'Ouvir a gravação';

  @override
  String get stopRecordingPlayback => 'Parar a reprodução';

  @override
  String get recordingCouldNotBePlayed =>
      'Não foi possível reproduzir a gravação.';

  @override
  String get columnWidth => 'Largura da coluna';

  @override
  String get subprojects => 'Subprojetos';

  @override
  String get newSubproject => 'Novo subprojeto';

  @override
  String get moveProjectTo => 'Mover para…';

  @override
  String get topLevel => 'Nível superior';

  @override
  String moveProjectTitle(String project) {
    return 'Mover $project para';
  }

  @override
  String get movedToTopLevel => 'Movido para o nível superior';

  @override
  String get theProjectCouldNotBeMoved => 'Não foi possível mover o projeto.';

  @override
  String subprojectCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count subprojetos',
      one: '1 subprojeto',
    );
    return '$_temp0';
  }

  @override
  String get chooseAProject =>
      'Escolha um projeto para ver as suas reuniões e ficheiros.';

  @override
  String get chooseAMeetingOrFile =>
      'Escolha uma reunião ou um ficheiro para o ver aqui.';

  @override
  String get backgroundPassTitle =>
      'Melhorar as transcrições com o Koalino fechado';

  @override
  String get backgroundPassAllowed =>
      'Permitido: o telefone melhora as transcrições pendentes enquanto carrega, mesmo com o Koalino fechado ou a economia de energia ativada.';

  @override
  String get backgroundPassAsk =>
      'Toque para que o Android faça isso enquanto o telefone carrega, mesmo com a economia de energia ativada. Caso contrário, só acontece com o Koalino aberto.';

  @override
  String get chatPrompt => 'Prompt do chat';

  @override
  String chatPromptHelp(String placeholder) {
    return '$placeholder representa aquilo sobre o que você pergunta: uma reunião, um projeto ou todas as suas notas. As respostas sempre citam suas fontes por número, e suas notas vêm depois do prompt.';
  }

  @override
  String get saveChatPrompt => 'Salvar prompt do chat';

  @override
  String get chatPromptSaved => 'Prompt do chat salvo.';

  @override
  String get theBuiltInChatPromptIsBack => 'O prompt do chat padrão voltou.';

  @override
  String get promptsSharedWithPairedDevices =>
      'Os dois prompts são iguais em todos os seus dispositivos pareados: uma alteração aqui chega a eles quando se conectam.';

  @override
  String get deviceOnline => 'Online';

  @override
  String get deviceOffline => 'Offline';

  @override
  String get neverSynced => 'Nunca sincronizado';

  @override
  String get syncedJustNow => 'Sincronizado agora mesmo';

  @override
  String lastSyncedMinutesAgo(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Última sincronização há $count minutos',
      one: 'Última sincronização há 1 minuto',
    );
    return '$_temp0';
  }

  @override
  String lastSyncedHoursAgo(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Última sincronização há $count horas',
      one: 'Última sincronização há 1 hora',
    );
    return '$_temp0';
  }

  @override
  String lastSyncedOn(String date) {
    return 'Última sincronização: $date';
  }

  @override
  String devicesOnlineCount(int online, int total) {
    return '$online de $total online';
  }

  @override
  String get microphoneSeemsMutedOrOff =>
      'O microfone parece silenciado ou desligado: nada está a ser ouvido. Verifique o botão ou a tecla de silêncio e se o acesso ao microfone está ativado nas definições do sistema.';

  @override
  String get windowsBlocksTheMicrophone =>
      'O Windows está a impedir as apps de usar o microfone: nada está a ser ouvido. Em Definições → Privacidade e segurança → Microfone, ative o Acesso ao microfone e Permitir que as aplicações de ambiente de trabalho acedam ao microfone.';

  @override
  String get backUpAllData => 'Fazer backup de todos os dados';

  @override
  String get backUpAllDataDescription =>
      'Salva reuniões, notas, anexos e gravações guardadas num único arquivo criptografado';

  @override
  String get restoreFromBackup => 'Restaurar de um backup';

  @override
  String get restoreFromBackupDescription =>
      'Substitui os dados deste dispositivo por um backup';

  @override
  String get backupPassphraseTitle => 'Frase-senha do backup';

  @override
  String get backupPassphraseExplanation =>
      'O backup é criptografado com esta frase-senha. Sem ela ninguém pode abri-lo, nem você, por isso guarde-a num lugar seguro.';

  @override
  String get backupPassphraseLabel => 'Frase-senha';

  @override
  String get repeatPassphraseLabel => 'Repita a frase-senha';

  @override
  String get passphraseTooShort => 'Use pelo menos 8 caracteres.';

  @override
  String get passphrasesDoNotMatch => 'As frases-senha não coincidem.';

  @override
  String get continueAction => 'Continuar';

  @override
  String get backingUp => 'Fazendo backup…';

  @override
  String get restoringBackup => 'Restaurando…';

  @override
  String backupSaved(int files, String size) {
    return 'Backup salvo: $files arquivos, $size MB.';
  }

  @override
  String get restoreConfirmTitle => 'Substituir os dados deste dispositivo?';

  @override
  String get restoreConfirmBody =>
      'As reuniões e notas deste dispositivo são substituídas pelas do backup. Os dados atuais ficam guardados à parte até à próxima restauração. Depois o Koalino reinicia.';

  @override
  String get restoreAction => 'Restaurar';

  @override
  String restoreReady(int files) {
    return '$files arquivos restaurados. O Koalino reinicia agora para abri-los.';
  }

  @override
  String get restartNow => 'Reiniciar';

  @override
  String get backupWrongPassphrase => 'A frase-senha está errada.';

  @override
  String get backupNotABackup => 'Este arquivo não é um backup do Koalino.';

  @override
  String get backupTooNew =>
      'Este backup foi feito por uma versão mais recente do Koalino. Atualize primeiro o app.';

  @override
  String get backupDamaged => 'O backup está danificado ou incompleto.';

  @override
  String get backupFailed =>
      'Não foi possível concluir a operação. Verifique o espaço livre e tente de novo.';

  @override
  String get automaticBackups => 'Backups automáticos';

  @override
  String get automaticBackupsDescription =>
      'Todos os dias durante o carregamento, numa pasta à sua escolha, também na nuvem (Google Drive, Nextcloud, OneDrive…). Criptografado antes de sair do telefone.';

  @override
  String automaticBackupFolder(String folder) {
    return 'Pasta: $folder';
  }

  @override
  String get noAutomaticBackupYet => 'Ainda não há backups automáticos.';

  @override
  String lastAutomaticBackup(String time) {
    return 'Último backup: $time';
  }

  @override
  String lastAutomaticBackupFailed(String time) {
    return 'O backup de $time falhou. Verifique a pasta.';
  }

  @override
  String get backUpNow => 'Fazer backup agora';

  @override
  String get backupFolderUnavailable =>
      'Não é possível gravar na pasta de backups. Verifique se ela ainda existe e se o app dela tem sessão iniciada, ou escolha outra pasta.';

  @override
  String get automaticBackupsNotSetUp =>
      'Os backups automáticos precisam ser configurados de novo: desative e ative novamente.';

  @override
  String get renameSpeaker => 'Mudar o nome de quem fala';

  @override
  String get nameThisVoice => 'Dar nome a esta voz';

  @override
  String get doubleClickOrLongPressToRename =>
      'Clique duas vezes ou mantenha pressionado para mudar o nome';

  @override
  String get theNewNameAppliesToEveryPassage =>
      'O novo nome vale para cada trecho desta voz, em todas as suas reuniões e nos seus dispositivos emparelhados.';

  @override
  String anotherSavedPersonIsCalled(Object name) {
    return 'Outra pessoa salva já se chama “$name”. Se for a mesma pessoa, junte as duas.';
  }

  @override
  String get useOneTo100CharactersOnOneLine =>
      'Use de 1 a 100 caracteres em uma linha.';

  @override
  String versionNumber(Object version) {
    return 'Versão $version';
  }

  @override
  String get developerSettings => 'Configurações de desenvolvedor';

  @override
  String get appVariablesForExpertsAndDevelopers =>
      'Variáveis do app, para especialistas e desenvolvedores';

  @override
  String tapsLeftToTurnOnDeveloperSettings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Mais $count toques para ativar as configurações de desenvolvedor',
      one: 'Mais 1 toque para ativar as configurações de desenvolvedor',
    );
    return '$_temp0';
  }

  @override
  String get developerSettingsAreOn =>
      'As configurações de desenvolvedor estão ativadas';

  @override
  String get developerSettingsWarning =>
      'Estas variáveis mudam o funcionamento interno do Koalino. Um valor errado pode parar a transcrição, a sincronização ou a extensão do navegador. Os valores salvos valem quando o Koalino reinicia.';

  @override
  String get restartKoalinoToApply =>
      'Reinicie o Koalino para aplicar os valores salvos.';

  @override
  String nowAndDefault(Object now, Object defaultValue) {
    return 'Agora: $now · Padrão: $defaultValue';
  }

  @override
  String get notSet => 'não definido';

  @override
  String savedForTheNextStart(Object value) {
    return 'Salvo para a próxima inicialização: $value';
  }

  @override
  String get useDefault => 'Usar padrão';

  @override
  String get resetAll => 'Redefinir tudo';

  @override
  String get turnOffDeveloperSettings =>
      'Desativar configurações de desenvolvedor';

  @override
  String get turnOffDeveloperSettingsExplanation =>
      'Cada valor volta ao padrão na próxima inicialização, e esta seção fica oculta de novo.';

  @override
  String get turnOff => 'Desativar';

  @override
  String mergeIntoAnotherPerson(Object name) {
    return 'Juntar $name a outra pessoa';
  }

  @override
  String mergeNameInto(Object name) {
    return 'Juntar $name a…';
  }

  @override
  String mergeWarning(Object source, Object target) {
    return 'Todos os trechos e amostras de voz de $source passam para $target, e $source é excluído, também nos dispositivos emparelhados. Isso não pode ser desfeito.';
  }

  @override
  String get merge => 'Juntar';

  @override
  String mergeWithName(Object name) {
    return 'Juntar a $name';
  }

  @override
  String mergedInto(Object source, Object target) {
    return '$source agora faz parte de $target';
  }

  @override
  String get thesePeopleCouldNotBeMerged =>
      'Não foi possível juntar essas pessoas. Tente novamente.';

  @override
  String get peopleCannotBeMergedWhileRecording =>
      'Não é possível juntar pessoas enquanto uma reunião está sendo gravada.';

  @override
  String get notEnoughFreeSpaceToDownload =>
      'Ainda não há espaço livre suficiente para o transferir.';

  @override
  String get downloadTheBestModelByItself => 'Baixar sozinho o melhor modelo';

  @override
  String get downloadTheBestModelByItselfHint =>
      'Com Wi-Fi e se couber, o modelo que o Automático escolhe para este dispositivo é baixado ao iniciar. Enquanto isso, as gravações usam o modelo atual.';

  @override
  String gettingAMoreAccurateModel(String name, String percent) {
    return 'Baixando um modelo mais preciso ($name) para este dispositivo: $percent';
  }

  @override
  String get recordingsUseTheCurrentModelMeanwhile =>
      'As gravações continuam usando o modelo atual até ele ficar pronto.';

  @override
  String get youCanRecordOnceItIsReady =>
      'Você poderá gravar assim que ele estiver pronto.';

  @override
  String get skipBack10Seconds => 'Voltar 10 segundos';

  @override
  String get skipForward10Seconds => 'Avançar 10 segundos';

  @override
  String get playbackSpeed => 'Velocidade de reprodução';

  @override
  String get followTranscript => 'Acompanhar na transcrição';

  @override
  String get playFromHere => 'Reproduzir a partir daqui';

  @override
  String get openingTheRecording => 'Abrindo a gravação…';

  @override
  String get recordingPlayer => 'Player da gravação';
}
