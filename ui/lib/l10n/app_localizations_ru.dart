// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Russian (`ru`).
class AppLocalizationsRu extends AppLocalizations {
  AppLocalizationsRu([String locale = 'ru']) : super(locale);

  @override
  String get unableToCheckTheAppLock =>
      'Не удалось проверить блокировку приложения. Попробуйте ещё раз.';

  @override
  String get unableToShowTheUnlockPrompt =>
      'Не удалось показать запрос разблокировки. Попробуйте ещё раз.';

  @override
  String get setAScreenLockOnThis =>
      'Сначала настройте блокировку экрана на этом устройстве.';

  @override
  String get unableToChangeTheAppLock =>
      'Не удалось изменить блокировку приложения. Попробуйте ещё раз.';

  @override
  String get aCallApp => 'приложение для звонков';

  @override
  String get call => 'Звонок';

  @override
  String call2(Object app) {
    return 'Звонок в $app';
  }

  @override
  String theCallEndedSoTheRecording(Object tiedApp) {
    return 'Звонок в $tiedApp завершён, поэтому запись остановлена.';
  }

  @override
  String get restartYourBrowserAndCallApps =>
      'Перезапустите браузер и приложения для звонков один раз, чтобы Koalino мог видеть, кто говорит во время звонка.';

  @override
  String get unableToLoadDevices => 'Не удалось загрузить устройства';

  @override
  String get thisDeviceIsNotOnA =>
      'Это устройство ещё не подключено к сети. Подключитесь и попробуйте снова.';

  @override
  String get thatDidNotWorkTryAgain => 'Не получилось. Попробуйте ещё раз.';

  @override
  String get use1To64CharactersWithout =>
      'Используйте от 1 до 64 символов без переносов строк.';

  @override
  String get theNameCouldNotBeSaved =>
      'Не удалось сохранить имя. Попробуйте ещё раз.';

  @override
  String get thisInviteHasExpiredMakeA =>
      'Срок действия приглашения истёк. Создайте новое на другом устройстве.';

  @override
  String get thisInviteWasMadeOnThis =>
      'Это приглашение создано на этом устройстве. Используйте его на другом устройстве.';

  @override
  String get thatDeviceIsAlreadyPaired => 'Это устройство уже сопряжено.';

  @override
  String get thatIsNotAValidInvite =>
      'Это недействительное приглашение. Скопируйте его ещё раз.';

  @override
  String get deviceSyncIsNotRunning => 'Синхронизация устройств не запущена.';

  @override
  String get theOtherDeviceIsNoLonger =>
      'Другое устройство больше не выполняет сопряжение. Попробуйте ещё раз.';

  @override
  String get theOtherAppCouldNotBe => 'Не удалось открыть другое приложение.';

  @override
  String get theOtherApp => 'другое приложение';

  @override
  String app(Object displayName) {
    return 'приложение $displayName';
  }

  @override
  String get unableToChangeTheLogLevel => 'Не удалось изменить уровень журнала';

  @override
  String logsExportedTo(Object filePath) {
    return 'Журналы экспортированы в $filePath';
  }

  @override
  String get unableToShareTheLogs => 'Не удалось поделиться журналами';

  @override
  String get noLogsToExportYet => 'Пока нет журналов для экспорта';

  @override
  String get unableToExportTheLogs => 'Не удалось экспортировать журналы';

  @override
  String couldNotBeAttached(Object attached, Object failedPaths) {
    return '$attached; не удалось прикрепить: $failedPaths';
  }

  @override
  String get theFilesCouldNotBeAttached => 'Не удалось прикрепить файлы.';

  @override
  String get unableToShareTheExport => 'Не удалось поделиться экспортом';

  @override
  String exportedTo(Object filePath) {
    return 'Экспортировано в $filePath';
  }

  @override
  String get exportSaved => 'Экспорт сохранён';

  @override
  String get unableToSaveTheExport => 'Не удалось сохранить экспорт';

  @override
  String get thisMeetingNoLongerExists => 'Эта встреча больше не существует';

  @override
  String get exportIsUnavailableUntilTheApp =>
      'Экспорт недоступен, пока приложение не запустится';

  @override
  String get unableToExportTheMeeting => 'Не удалось экспортировать встречу';

  @override
  String get unableToLoadMeetings => 'Не удалось загрузить встречи';

  @override
  String get unableToLoadThisMeeting => 'Не удалось загрузить эту встречу';

  @override
  String get enterATitle => 'Введите название.';

  @override
  String titlesCanBeAtMostCharacters(
      Object maxMeetingTitleCharacters, Object runes) {
    return 'Название может содержать не более $maxMeetingTitleCharacters символов (сейчас $runes).';
  }

  @override
  String get titlesCanTContainLineBreaks =>
      'Название не может содержать переносы строк или управляющие символы.';

  @override
  String get notPairedYetMakeTheRelay =>
      'Ещё не сопряжено. Сделайте ретранслятор видимым и выполните сопряжение в разделе «Устройства» → «Устройства поблизости».';

  @override
  String get connectingToTheRelay => 'Подключение к ретранслятору…';

  @override
  String get connectedToTheRelayAskingIt =>
      'Подключено к ретранслятору, запрашиваем приём подключений…';

  @override
  String get connectedYourOtherDevicesCanReach =>
      'Подключено. Другие ваши устройства могут связаться с этим через ретранслятор.';

  @override
  String get savedThisAppliesNowWithNo =>
      'Сохранено. Изменения применяются сразу, без перезапуска.';

  @override
  String get thatRelayAddressIsNotValid =>
      'Недопустимый адрес ретранслятора. Он выглядит так: /ip4/203.0.113.5/tcp/4001/p2p/12D3Koo…';

  @override
  String get aDeviceAddressIsNotValid =>
      'Один из адресов устройств недопустим. Каждый должен заканчиваться на /p2p/ и ID устройства.';

  @override
  String get unableToSaveTheAddresses => 'Не удалось сохранить адреса.';

  @override
  String get thePhotoCouldNotBeSaved => 'Не удалось сохранить фото.';

  @override
  String get enterAName => 'Введите имя.';

  @override
  String namesCanBeAtMostCharacters(Object maxMeetingTitleCharacters) {
    return 'Имя может содержать не более $maxMeetingTitleCharacters символов.';
  }

  @override
  String get namesCanTContainLineBreaks =>
      'Имя не может содержать переносы строк или управляющие символы.';

  @override
  String get unableToLoadProjects => 'Не удалось загрузить проекты';

  @override
  String get unableToLoadThisProject => 'Не удалось загрузить этот проект';

  @override
  String get theFilesCouldNotBeAdded => 'Не удалось добавить файлы.';

  @override
  String couldNotBeAdded(Object failedPaths) {
    return 'не удалось добавить: $failedPaths';
  }

  @override
  String get unableToStartRecording => 'Не удалось начать запись';

  @override
  String get unableToPauseRecording => 'Не удалось приостановить запись';

  @override
  String get unableToResumeRecording => 'Не удалось возобновить запись';

  @override
  String get unableToStopRecording => 'Не удалось остановить запись';

  @override
  String get recordingSaved => 'Запись сохранена';

  @override
  String get microphoneAccessIsNeededToRecord =>
      'Для записи нужен доступ к микрофону';

  @override
  String get keepTheAppOpenAndroidDid =>
      'Не закрывайте приложение: Android не разрешил запись в фоне';

  @override
  String get unableToOpenTheMicrophone => 'Не удалось открыть микрофон';

  @override
  String saved(Object label) {
    return '$label сохранено.';
  }

  @override
  String removed(Object label) {
    return '$label удалено.';
  }

  @override
  String isNowTheDefault(Object id) {
    return '$id теперь используется по умолчанию.';
  }

  @override
  String answered(Object id, Object reply) {
    return '$id ответил: $reply';
  }

  @override
  String get promptSaved => 'Промпт сохранён.';

  @override
  String get theBuiltInPromptIsBack => 'Восстановлен встроенный промпт.';

  @override
  String get finishedMeetingsWillBeSummarizedAutomatically =>
      'Завершённые встречи будут резюмироваться автоматически.';

  @override
  String get automaticSummariesAreOff => 'Автоматические резюме отключены.';

  @override
  String get thisComputerNoLongerWritesSummaries =>
      'Этот компьютер больше не пишет резюме и не отвечает в чатах для других устройств.';

  @override
  String nowWritesTheSummariesAndAnswers(Object id) {
    return '$id теперь пишет резюме и отвечает в чатах по запросу других устройств.';
  }

  @override
  String get requestCancelled => 'Запрос отменён.';

  @override
  String get thisMeetingUsesTheSettingsPrompt =>
      'Эта встреча снова использует промпт из настроек.';

  @override
  String get promptSavedForThisMeeting => 'Промпт для этой встречи сохранён.';

  @override
  String get summarySaved => 'Резюме сохранено.';

  @override
  String get theConnection => 'подключение';

  @override
  String get summaryWritten => 'Резюме готово.';

  @override
  String theTranscriptWasReadInParts(Object parts) {
    return 'Расшифровка прочитана по частям: $parts.';
  }

  @override
  String leftOut(Object sent, Object imagesSkipped) {
    return '$sent; пропущено: $imagesSkipped.';
  }

  @override
  String get theModelStoppedBeforeTheEnd =>
      'Модель остановилась, не дойдя до конца.';

  @override
  String get waitingForYourComputerItWrites =>
      'Ожидание вашего компьютера: он напишет резюме, когда будет включён, подключён к этому устройству и настроен отвечать в «Настройки → ИИ-подключения».';

  @override
  String get yourComputerIsWritingTheSummary => 'Ваш компьютер пишет резюме…';

  @override
  String yourComputerCouldNotWriteThe(Object message) {
    return 'Компьютер не смог написать резюме: $message';
  }

  @override
  String get unableToRefreshTelemetry => 'Не удалось обновить телеметрию';

  @override
  String get automatic => 'Автоматически';

  @override
  String get transcriptionLanguage => 'Язык расшифровки';

  @override
  String get transcriptionLanguages => 'Языки расшифровки';

  @override
  String get theLanguageCouldNotBeChanged => 'Не удалось изменить язык.';

  @override
  String get thisDevice => 'Это устройство';

  @override
  String get bestAvailable => 'Лучшее доступное';

  @override
  String get aRemovedDevice => 'Удалённое устройство';

  @override
  String get theTranscriptionDeviceCouldNotBe =>
      'Не удалось изменить устройство для расшифровки. Проверьте, что оно всё ещё сопряжено.';

  @override
  String get downloadFinishedYouCanNowChoose =>
      'Загрузка завершена. Теперь эту модель можно выбрать.';

  @override
  String get downloadCancelled => 'Загрузка отменена.';

  @override
  String get theDownloadStoppedNoConnectionTo =>
      'Загрузка остановлена: нет связи с сервером. Текущая модель продолжает работать.';

  @override
  String get theDownloadedFileFailedItsCheck =>
      'Загруженный файл не прошёл проверку и был удалён. Текущая модель продолжает работать.';

  @override
  String get theModelCouldNotBeSaved =>
      'Не удалось сохранить модель: проверьте свободное место. Текущая модель продолжает работать.';

  @override
  String get theDownloadServerRefusedTheFile =>
      'Сервер загрузки отказал в выдаче файла. Текущая модель продолжает работать.';

  @override
  String get savedTheNextRecordingUsesThis =>
      'Сохранено. Следующая запись использует эту модель.';

  @override
  String get savedTheNextRecordingUsesThis2 =>
      'Сохранено. Применится к следующей записи.';

  @override
  String get savedTheNextRecordingUsesThe =>
      'Сохранено. Следующая запись использует автоматический выбор.';

  @override
  String get savedMeetingsRecordedFromNowOn =>
      'Сохранено. Применится к встречам, записанным с этого момента.';

  @override
  String get savedTheNextRecordingUsesThese =>
      'Сохранено. Следующая запись использует эти имена и термины.';

  @override
  String get eachTermCanHaveAtMost =>
      'Каждый термин — не более 60 символов, всего — не более 500.';

  @override
  String get chooseAWindowBetween1And => 'Выберите срок от 1 до 60 дней.';

  @override
  String get theDownloadCouldNotStartPlease =>
      'Не удалось начать загрузку. Попробуйте ещё раз.';

  @override
  String get savedTheNextRecordingIsTranscribed =>
      'Сохранено. Следующая запись будет расшифрована на NPU.';

  @override
  String get savedTheNextRecordingIsTranscribed2 =>
      'Сохранено. Следующая запись будет расшифрована на процессоре.';

  @override
  String get theNpuFilesAreNotAll => 'Файлы для NPU загружены не полностью.';

  @override
  String get theNpuFilesWereRemoved => 'Файлы для NPU удалены.';

  @override
  String get theNpuFilesCouldNotBe =>
      'Не удалось удалить файлы для NPU. Попробуйте ещё раз.';

  @override
  String get downloadedRecordingsNowTranscribeOnThe =>
      'Загружено. Теперь записи расшифровываются на NPU.';

  @override
  String get theDownloadStoppedNoConnectionTo2 =>
      'Загрузка остановлена: нет связи с сервером.';

  @override
  String get aDownloadedFileFailedItsCheck =>
      'Один из загруженных файлов не прошёл проверку и был удалён.';

  @override
  String get theFilesCouldNotBeSaved =>
      'Не удалось сохранить файлы: проверьте свободное место (нужно около 800 МБ).';

  @override
  String get theDownloadServerRefusedTheFiles =>
      'Сервер загрузки отказал в выдаче файлов.';

  @override
  String get theNextRecordingTriesTheGraphics =>
      'Следующая запись снова попробует графический чип.';

  @override
  String get thatModelWonTBeProposed =>
      'Эта модель больше не будет предлагаться.';

  @override
  String get modelRemoved => 'Модель удалена.';

  @override
  String get anotherModelIsDownloadingWaitFor =>
      'Загружается другая модель. Дождитесь окончания.';

  @override
  String get thereIsnTEnoughFreeSpace =>
      'Недостаточно свободного места для этой модели.';

  @override
  String get thatDidnTWorkPleaseTry => 'Не получилось. Попробуйте ещё раз.';

  @override
  String get unableToLoadVoiceGallery => 'Не удалось загрузить галерею голосов';

  @override
  String get unableToPlayThisVoice => 'Не удалось воспроизвести этот голос';

  @override
  String get unableToDeleteThisVoice => 'Не удалось удалить этот голос';

  @override
  String get voiceDeleted => 'Голос удалён';

  @override
  String get unableToDeleteThisPerson => 'Не удалось удалить этого человека';

  @override
  String deleted(Object displayName) {
    return '$displayName удалён(а)';
  }

  @override
  String get enterANameBeforeAssigning => 'Введите имя перед назначением';

  @override
  String assignedTo(Object identityName) {
    return 'Назначено: $identityName';
  }

  @override
  String assignedToThereWasNotEnough(Object identityName) {
    return 'Назначено: $identityName. Чёткой речи было недостаточно, чтобы узнавать этот голос в будущих записях.';
  }

  @override
  String get unableToAssignTheSpeakerCluster =>
      'Не удалось назначить группу говорящего';

  @override
  String get chooseAtLeastOneVoiceTo =>
      'Выберите хотя бы один голос для назначения имени';

  @override
  String thereWasNotEnoughClearSpeech(Object assigned) {
    return '$assigned. Чёткой речи было недостаточно, чтобы узнавать этот голос в будущих записях.';
  }

  @override
  String get theSettingCouldNotBeSaved => 'Не удалось сохранить настройку.';

  @override
  String get theAppIsStillStartingTry =>
      'Приложение ещё запускается. Попробуйте ещё раз.';

  @override
  String get somethingWentWrongTryAgain =>
      'Что-то пошло не так. Попробуйте ещё раз.';

  @override
  String isUsingTheMicrophone(Object app) {
    return '$app использует микрофон';
  }

  @override
  String get recordThisCall => 'Записать этот звонок?';

  @override
  String theAiServerRejectedTheRequest(Object detail) {
    return 'ИИ-сервер отклонил запрос.\nОтвет сервера: $detail';
  }

  @override
  String claudeCodeOrCodexCouldNot(Object detail) {
    return 'Claude Code или Codex не смог ответить.\nОтвет: $detail';
  }

  @override
  String get typeAQuestion => 'Введите вопрос.';

  @override
  String get theQuestionIsTooLong => 'Вопрос слишком длинный.';

  @override
  String get addAnAiConnectionInSettings =>
      'Добавьте ИИ-подключение в настройках или выберите подключение по умолчанию.';

  @override
  String get thatAiConnectionNoLongerExists =>
      'Это ИИ-подключение больше не существует.';

  @override
  String get thereIsNothingToTalkAbout =>
      'Пока не о чем говорить: нет ни расшифровки, ни резюме, ни заметок, ни файлов.';

  @override
  String get theAiConnectionSContextIs =>
      'Контекст ИИ-подключения слишком мал. Выберите подключение с большим контекстом.';

  @override
  String get thatMeetingNoLongerExists => 'Эта встреча больше не существует.';

  @override
  String get thatProjectNoLongerExists => 'Этот проект больше не существует.';

  @override
  String get theAiServerCouldNotBe =>
      'Не удалось связаться с ИИ-сервером. Проверьте подключение.';

  @override
  String get theAiServerRefusedTheApi => 'ИИ-сервер отклонил API-ключ.';

  @override
  String get theAiServerIsBusyTry => 'ИИ-сервер занят. Попробуйте позже.';

  @override
  String get theModelDeclinedToAnswer => 'Модель отказалась отвечать.';

  @override
  String get claudeCodeOrCodexIsNot =>
      'Claude Code или Codex не установлен на этом компьютере, или приложение не может его найти.';

  @override
  String get theNotesCouldNotBeRead => 'Не удалось прочитать заметки.';

  @override
  String get noPairedComputerAnsweredOnA =>
      'Ни один сопряжённый компьютер не ответил. На компьютере откройте «Настройки → ИИ», выберите подключение в «Резюме для других моих устройств» и держите его включённым и подключённым.';

  @override
  String get theComputerTookTooLongTo => 'Компьютер отвечал слишком долго.';

  @override
  String get theComputerDisconnectedBeforeAnsweringTry =>
      'Компьютер отключился до ответа. Попробуйте ещё раз.';

  @override
  String get theComputerAnswersAtMost30 =>
      'Компьютер отвечает не более чем на 30 вопросов в час. Попробуйте позже.';

  @override
  String get theComputerIsAnsweringOtherQuestions =>
      'Компьютер отвечает на другие вопросы. Попробуйте чуть позже.';

  @override
  String get pairedDevicesAreOffOnThis =>
      'Сопряжённые устройства отключены на этом телефоне.';

  @override
  String get theAiServerCouldNotAnswer =>
      'ИИ-сервер не смог ответить. Попробуйте ещё раз.';

  @override
  String get exportMeeting => 'Экспорт встречи';

  @override
  String get openaiCompatibleServer => 'OpenAI-совместимый сервер';

  @override
  String get localServerOllama => 'Локальный сервер (Ollama)';

  @override
  String get claudeCodeClaudePlan => 'Claude Code (план Claude)';

  @override
  String get codexChatgptPlan => 'Codex (план ChatGPT)';

  @override
  String get checkTheAddressAndModelPlain =>
      'Проверьте адрес и модель. Незашифрованный http:// работает только на этом устройстве или в локальной сети.';

  @override
  String get thatConnectionNoLongerExists =>
      'Это подключение больше не существует.';

  @override
  String get addAnAiConnectionInSettings2 =>
      'Сначала добавьте ИИ-подключение в настройках.';

  @override
  String get thisMeetingHasNoTranscriptOr =>
      'У этой встречи пока нет расшифровки или заметок.';

  @override
  String get thisMeetingIsTooLongFor =>
      'Эта встреча слишком длинная для этого подключения. Выберите подключение с большим контекстом.';

  @override
  String get theAiServerCouldNotBe2 => 'Не удалось связаться с ИИ-сервером.';

  @override
  String get theAiServerRefusedAccessUse =>
      'ИИ-сервер отказал в доступе. Проверьте API-ключ подключения.';

  @override
  String get theAiServerIsBusyOr =>
      'ИИ-сервер занят или превышен лимит. Попробуйте позже.';

  @override
  String get theAiServerRejectedTheRequest2 =>
      'ИИ-сервер отклонил запрос. Проверьте название модели.';

  @override
  String get theAiServerFailedTryAgain =>
      'Ошибка ИИ-сервера. Попробуйте ещё раз.';

  @override
  String get theAiServerSAnswerCould =>
      'Не удалось прочитать ответ ИИ-сервера.';

  @override
  String get claudeCodeOrCodexCouldNot2 =>
      'Claude Code или Codex не смог ответить. Проверьте, что вы вошли в систему.';

  @override
  String get theSettingsCouldNotBeSaved => 'Не удалось сохранить настройки.';

  @override
  String get theMeetingCouldNotBeRead =>
      'Не удалось прочитать или сохранить встречу.';

  @override
  String get error => 'Ошибка';

  @override
  String get warning => 'Предупреждение';

  @override
  String get infoDefault => 'Информация (по умолчанию)';

  @override
  String get debug => 'Отладка';

  @override
  String get trace => 'Трассировка';

  @override
  String get thisDevice2 => 'это устройство';

  @override
  String get anotherDevice => 'другое устройство';

  @override
  String notConnected(Object label) {
    return '$label (не подключено)';
  }

  @override
  String itIsKeptEncryptedUntil(Object until) {
    return 'Хранится в зашифрованном виде до $until.';
  }

  @override
  String get recordedOnThisDeviceOnly => 'Записано только на этом устройстве.';

  @override
  String recordedOnThisDeviceOnlyA(Object until) {
    return 'Записано только на этом устройстве. Запись той же встречи на другом устройстве можно объединить до $until.';
  }

  @override
  String combinesTheRecordingsNothingIsLeft(Object combiner) {
    return '$combiner объединяет записи; отсюда отправлять больше нечего.';
  }

  @override
  String waitingForAudioFrom(Object names, Object kept) {
    return 'Ожидание аудио от: $names. $kept';
  }

  @override
  String combinesTheRecordingsWaitingForAudio(Object combiner, Object names) {
    return '$combiner объединяет записи; ожидание аудио от: $names.';
  }

  @override
  String get audioFromEveryDeviceIsHere =>
      'Аудио со всех устройств получено; это устройство объединит расшифровки, когда освободится.';

  @override
  String thisDeviceSAudioStillHas(Object combiner, Object kept) {
    return 'Аудио этого устройства ещё не доставлено на $combiner. $kept';
  }

  @override
  String get transcriptCombinedFromEveryDevice =>
      'Расшифровка объединена со всех устройств.';

  @override
  String notCombinedAudioFromDidNot(Object plainNames) {
    return 'Не объединено: аудио от $plainNames не поступило в течение трёх недель.';
  }

  @override
  String meeting(Object createdAt) {
    return 'Встреча $createdAt';
  }

  @override
  String get writeSomethingFirst => 'Сначала что-нибудь напишите.';

  @override
  String get theTextIsTooLong => 'Текст слишком длинный.';

  @override
  String get thatPartOfTheTranscriptChanged =>
      'Эта часть расшифровки тем временем изменилась. Откройте встречу заново.';

  @override
  String get theMeetingCouldNotBeSaved => 'Не удалось сохранить встречу.';

  @override
  String get chooseAFullFolderPathInside =>
      'Выберите полный путь к папке внутри домашней папки (но не саму домашнюю папку).';

  @override
  String get theFolderCouldNotBeWritten =>
      'Не удалось записать в папку. Проверьте, что она существует и вы можете в неё писать.';

  @override
  String get theNotesCouldNotBeRead2 =>
      'Не удалось прочитать или сохранить заметки.';

  @override
  String get theInviteAnswerIsIncomplete => 'Ответ на приглашение неполный.';

  @override
  String get theRenameAnswerIsIncomplete => 'Ответ на переименование неполный.';

  @override
  String get thisDeviceHasNoCamera => 'На этом устройстве нет камеры.';

  @override
  String get theCameraIsNotOpen => 'Камера не открыта.';

  @override
  String get title => 'Название';

  @override
  String get summary => 'Резюме';

  @override
  String get notes => 'Заметки';

  @override
  String get transcript => 'Расшифровка';

  @override
  String get projectName => 'Название проекта';

  @override
  String file(Object fileName) {
    return 'Файл $fileName';
  }

  @override
  String get typeWhatToLookFor => 'Введите, что искать.';

  @override
  String get typeAtLeastTwoLetters => 'Введите хотя бы две буквы.';

  @override
  String get theSearchIsTooLong => 'Поисковый запрос слишком длинный.';

  @override
  String get exportLogs => 'Экспорт журналов';

  @override
  String about(Object appName) {
    return 'О приложении $appName';
  }

  @override
  String get versionLicenseAndTheSoftwareIt =>
      'Версия, лицензия и используемое ПО.';

  @override
  String remove(Object label) {
    return 'Удалить $label?';
  }

  @override
  String get itsApiKeyIsDeletedFrom =>
      'API-ключ будет удалён с этого устройства. Уже написанные резюме сохранятся.';

  @override
  String get cancel => 'Отмена';

  @override
  String get remove2 => 'Удалить';

  @override
  String get aiConnections => 'ИИ-подключения';

  @override
  String get addConnection => 'Добавить подключение';

  @override
  String get summariesSendTheMeetingSTranscript =>
      'Для резюме расшифровка, заметки и фото встречи отправляются на сервер подключения. Сервер в вашей локальной сети оставляет данные у вас.';

  @override
  String get noConnectionsYet => 'Подключений пока нет';

  @override
  String get addAServerOnYourNetwork =>
      'Добавьте сервер в своей сети (Ollama, LM Studio, llama.cpp) или Claude Code / Codex на этом компьютере.';

  @override
  String get summarizeMeetingsAutomatically =>
      'Автоматически резюмировать встречи';

  @override
  String get withTheDefaultConnectionWhenA =>
      'С помощью подключения по умолчанию, когда расшифровка встречи готова.';

  @override
  String get summariesForMyOtherDevices => 'Резюме для других моих устройств';

  @override
  String get aPairedPhoneCanAskThis =>
      'Сопряжённый телефон может попросить этот компьютер написать резюме или ответить в чате через это подключение, используя копию заметок на компьютере. Это расходует лимит вашего плана: до 10 резюме и 30 ответов в чате в час.';

  @override
  String get off => 'Выкл.';

  @override
  String get summaryPrompt => 'Промпт для резюме';

  @override
  String placeholdersTheTranscriptIsAlwaysSent(Object placeholders) {
    return 'Подстановки: $placeholders. Расшифровка всегда отправляется после промпта.';
  }

  @override
  String get savePrompt => 'Сохранить промпт';

  @override
  String get resetToBuiltIn => 'Вернуть встроенный';

  @override
  String get defaultLabel => 'По умолчанию';

  @override
  String get installed => 'Установлено';

  @override
  String get notFoundOnThisComputer => 'Не найдено на этом компьютере';

  @override
  String get unencryptedHttpLocalNetworkOnly =>
      'Незашифрованный http, только локальная сеть';

  @override
  String get runsWithYourPlanSSign =>
      'Работает с входом в ваш план на этом компьютере';

  @override
  String actionsFor(Object label) {
    return 'Действия для $label';
  }

  @override
  String get makeDefault => 'Сделать по умолчанию';

  @override
  String get test => 'Проверить';

  @override
  String get edit => 'Изменить';

  @override
  String get enterANameAnAddressAnd => 'Введите имя, адрес и модель.';

  @override
  String get theContextIsANumberOf =>
      'Контекст — это количество токенов, например 8192.';

  @override
  String edit2(Object label) {
    return 'Изменить $label';
  }

  @override
  String get name => 'Имя';

  @override
  String get protocol => 'Протокол';

  @override
  String get runsTheToolInstalledAndSigned =>
      'Запускает инструмент, установленный и авторизованный на этом компьютере, поэтому резюме пишутся в рамках вашего плана Claude или ChatGPT, а не за счёт API-кредитов. Встреча отправляется в Anthropic или OpenAI под этой учётной записью.';

  @override
  String get address => 'Адрес';

  @override
  String get httpsAlwaysHttpOnlyOnThis =>
      'https:// — всегда; http:// — только на этом устройстве или в локальной сети.';

  @override
  String get model => 'Модель';

  @override
  String get leaveEmptyForTheToolS =>
      'Оставьте пустым, чтобы использовать значение инструмента по умолчанию.';

  @override
  String get contextTokens => 'Контекст (токены)';

  @override
  String get saveConnection => 'Сохранить подключение';

  @override
  String get koalinoIsLocked => 'Koalino заблокирован';

  @override
  String get unlockWithYourFingerprintFaceOr =>
      'Разблокируйте отпечатком пальца, лицом или блокировкой экрана.';

  @override
  String get unlock => 'Разблокировать';

  @override
  String copied(Object what) {
    return '$what скопировано';
  }

  @override
  String get aiAssistantAccess => 'Доступ ИИ-ассистента';

  @override
  String get letYourOwnAiAssistantSuch =>
      'Разрешите своему ИИ-ассистенту, например Claude Code, Codex или Claude Desktop, читать встречи и проекты: названия, резюме, заметки, расшифровки и файлы проектов. Он может только читать — ничего не изменять и не удалять.';

  @override
  String get whatTheAssistantReadsMayBe =>
      'Прочитанное ассистентом может отправляться в онлайн-модель под вашей учётной записью этого сервиса. Включайте только для ассистентов, которым доверяете содержимое встреч.';

  @override
  String get theAssistantServerIsNotInstalled =>
      'Сервер ассистента не установлен вместе с этим приложением.';

  @override
  String get letMyAiAssistantReadMy =>
      'Разрешить моему ИИ-ассистенту читать мои заметки';

  @override
  String get connectYourAssistant => 'Подключите ассистента';

  @override
  String get yourNotesAreUnlockedWithA =>
      'Ваши заметки разблокируются парольной фразой. Она никогда не записывается в конфигурацию ассистента: сервер читает заметки, только если NOTE_TAKER_ENCRYPTION_PASSPHRASE задана в окружении, где ассистент его запускает.';

  @override
  String get claudeCodeRunInATerminal => 'Claude Code: выполните в терминале';

  @override
  String get command => 'Команда';

  @override
  String get claudeDesktopAndOtherMcpClients =>
      'Claude Desktop и другие MCP-клиенты: добавьте в их конфигурацию';

  @override
  String get settings => 'Настройки';

  @override
  String copy(Object title) {
    return 'Копировать $title';
  }

  @override
  String isUsingTheMicrophoneRecordThis(Object app) {
    return '$app использует микрофон. Записать этот звонок?';
  }

  @override
  String get notNow => 'Не сейчас';

  @override
  String get record => 'Записать';

  @override
  String get offerToRecordCalls => 'Предлагать запись звонков';

  @override
  String get whenTeamsZoomMeetInA =>
      'Когда Teams, Zoom, Meet в браузере или другое приложение для звонков начинает использовать микрофон. Ничего не записывается, пока вы не нажмёте «Записать».';

  @override
  String get stopWhenTheCallEnds => 'Останавливать по окончании звонка';

  @override
  String get onlyForRecordingsStartedFromThe =>
      'Только для записей, начатых из предложения: остановка через 20 секунд после того, как приложение освободит микрофон.';

  @override
  String get readSpeakerNamesFromTheCall =>
      'Считывать имена говорящих из окна звонка';

  @override
  String get whileRecordingKoalinoReadsWhoTeams =>
      'Во время записи Koalino считывает, кого Teams, Zoom или Meet показывают говорящим, и предлагает это имя для неизвестных голосов. Ничего не покидает этот компьютер, и никто не получает имя, пока вы не нажмёте «Назначить».';

  @override
  String get deleteThisChat => 'Удалить этот чат?';

  @override
  String get itsQuestionsAndAnswersAreDeleted =>
      'Вопросы и ответы будут удалены с этого устройства.';

  @override
  String get delete => 'Удалить';

  @override
  String get thatFileIsNoLongerIn => 'Этого файла больше нет в проекте.';

  @override
  String get whoAnswers => 'Кто отвечает';

  @override
  String get myPairedComputer => 'Мой сопряжённый компьютер';

  @override
  String get thisPhoneSAiConnection => 'ИИ-подключение этого телефона';

  @override
  String get deleteThisChat2 => 'Удалить этот чат';

  @override
  String get askAboutYourNotesAnswersCome =>
      'Спрашивайте о своих заметках. Ответы приходят с вашего сопряжённого компьютера: он читает свою копию заметок через подключение, выбранное для других устройств.';

  @override
  String get askAboutYourNotesTheAnswer =>
      'Спрашивайте о своих заметках. Ответ даёт ИИ-подключение по умолчанию; отправляется столько заметок, сколько поместится.';

  @override
  String get yourQuestion => 'Ваш вопрос';

  @override
  String get ask => 'Спросить';

  @override
  String get open => 'Открыть';

  @override
  String given(Object included) {
    return 'Передано: $included';
  }

  @override
  String leftOutForLength(Object omitted) {
    return 'Пропущено из-за длины: $omitted';
  }

  @override
  String answeredBy(Object connection) {
    return 'Ответил: $connection';
  }

  @override
  String get unableToOpenTheKeychain => 'Не удалось открыть связку ключей';

  @override
  String get koalinoKeepsTheKeyThatEncrypts =>
      'Koalino хранит ключ, шифрующий голосовые профили и идентификатор устройства, в системной связке ключей. Разблокируйте её или разрешите Koalino её использовать, затем попробуйте снова. Если на этом устройстве нет службы связки ключей, установите или запустите её и перезапустите Koalino.';

  @override
  String get unableToOpenYourNotes => 'Не удалось открыть заметки';

  @override
  String get koalinoCouldNotOpenItsDatabase =>
      'Koalino не смог открыть базу данных. Проверьте свободное место и попробуйте снова.';

  @override
  String get unlockingYourNotes => 'Разблокировка заметок…';

  @override
  String get quit => 'Выйти';

  @override
  String get retry => 'Повторить';

  @override
  String delete2(Object title) {
    return 'Удалить $title?';
  }

  @override
  String get itsTranscriptAttachmentsAndUnknownVoices =>
      'Расшифровка, вложения и неизвестные голоса будут удалены навсегда — здесь и на сопряжённых устройствах. Сохранённые люди останутся.';

  @override
  String get unableToDeleteThisMeetingIf =>
      'Не удалось удалить встречу. Если она записывается, сначала остановите запись.';

  @override
  String get devices => 'Устройства';

  @override
  String get deviceSyncIsOff => 'Синхронизация устройств выключена';

  @override
  String get meetingsStayOnThisDeviceUntil =>
      'Встречи остаются на этом устройстве, пока синхронизация не включена.';

  @override
  String thisDevice3(Object capabilities) {
    return 'Это устройство\n$capabilities';
  }

  @override
  String get renameThisDevice => 'Переименовать это устройство';

  @override
  String get discoverableYourOtherDevicesCanAsk =>
      'Видимо: в течение нескольких минут другие ваши устройства могут запросить сопряжение.';

  @override
  String get makeDiscoverable => 'Сделать видимым';

  @override
  String get pairingRequests => 'Запросы на сопряжение';

  @override
  String get acceptOnlyIfTheOtherDevice =>
      'Принимайте, только если другое устройство показывает тот же код.';

  @override
  String get reject => 'Отклонить';

  @override
  String get accept => 'Принять';

  @override
  String get pairingInProgress => 'Идёт сопряжение';

  @override
  String get waitingForTheOtherDevice => 'Ожидание другого устройства…';

  @override
  String get checkThatTheOtherDeviceShows =>
      'Проверьте, что другое устройство показывает тот же код.';

  @override
  String get cancelPairing => 'Отменить сопряжение';

  @override
  String get codesMatch => 'Коды совпадают';

  @override
  String get pairedDevices => 'Сопряжённые устройства';

  @override
  String get noPairedDevicesYet => 'Сопряжённых устройств пока нет.';

  @override
  String paired(Object pairedAt, Object hasRelay) {
    return 'Сопряжено $pairedAt\n$hasRelay';
  }

  @override
  String remove3(Object deviceName) {
    return 'Удалить $deviceName';
  }

  @override
  String get nearbyDevices => 'Устройства поблизости';

  @override
  String get pair => 'Сопрячь';

  @override
  String get anotherNetwork => 'Другая сеть';

  @override
  String get showAnInviteHereAndUse =>
      'Покажите здесь приглашение и используйте его на другом устройстве; перед сопряжением вы всё равно сверите коды.';

  @override
  String get showInvite => 'Показать приглашение';

  @override
  String get useInvite => 'Использовать приглашение';

  @override
  String stopsSyncingWithThisDevicePair(Object deviceName) {
    return '$deviceName перестанет синхронизироваться с этим устройством. Чтобы возобновить, выполните сопряжение снова.';
  }

  @override
  String pairingCode(Object split) {
    return 'Код сопряжения $split';
  }

  @override
  String get noOtherDevicesFoundOnThis =>
      'В этой сети других устройств не найдено. «Поблизости» находит только устройства в той же сети Wi-Fi; для устройства в другой сети используйте приглашение.';

  @override
  String get noOtherDevicesFoundOnThis2 =>
      'В этой сети других устройств не найдено. «Поблизости» находит только устройства в той же сети Wi-Fi; для устройства в другой сети используйте приглашение или укажите ретранслятор в настройках.';

  @override
  String get notConnectedItReconnectsWhenIt =>
      'Не подключено; переподключится, когда устройство будет в сети и сможет достичь ретранслятора.';

  @override
  String get notConnectedPutBothDevicesOn =>
      'Не подключено. Подключите оба устройства к одной сети Wi-Fi или укажите ретранслятор в настройках.';

  @override
  String get whisperOnDevice => 'Whisper (на устройстве)';

  @override
  String get openaiCloud => 'OpenAI (облако)';

  @override
  String gbGpu(Object round) {
    return '$round ГБ GPU';
  }

  @override
  String mbGpu(Object gpuMegabytes) {
    return '$gpuMegabytes МБ GPU';
  }

  @override
  String get noTranscriptionModels => 'Нет моделей расшифровки';

  @override
  String get useYourOwnAiApp => 'Использовать своё ИИ-приложение';

  @override
  String get theMeetingItsNotesAndIts =>
      'Встреча, её заметки и файлы отправляются в выбранное приложение под вашей учётной записью. Если нужно резюме, скопируйте там ответ и вернитесь или поделитесь им с Koalino: Koalino его сохранит.';

  @override
  String summarizeIn(Object displayName) {
    return 'Резюмировать в $displayName';
  }

  @override
  String chatIn(Object displayName) {
    return 'Чат в $displayName';
  }

  @override
  String get summarizeInAnotherApp => 'Резюмировать в другом приложении…';

  @override
  String get chatInAnotherApp => 'Чат в другом приложении…';

  @override
  String summaryFromSavedTo(Object appName, Object title) {
    return 'Резюме из $appName сохранено в «$title».';
  }

  @override
  String get undo => 'Отменить';

  @override
  String get theEarlierSummaryIsBack => 'Предыдущее резюме восстановлено.';

  @override
  String toKeepTheSummaryCopyS(Object appName) {
    return 'Чтобы сохранить резюме, скопируйте ответ $appName и вернитесь в Koalino или поделитесь ответом с Koalino.';
  }

  @override
  String get meetings => 'Встречи';

  @override
  String get projects => 'Проекты';

  @override
  String get voices => 'Голоса';

  @override
  String get meeting2 => 'Встреча';

  @override
  String get chatAboutThisMeeting => 'Чат об этой встрече';

  @override
  String chat(Object meeting) {
    return 'Чат: $meeting';
  }

  @override
  String get thisMeeting => 'Эта встреча';

  @override
  String get takePhotos => 'Сделать фото';

  @override
  String get attachFiles => 'Прикрепить файлы';

  @override
  String get export => 'Экспорт';

  @override
  String shareAs(Object label) {
    return 'Поделиться как $label';
  }

  @override
  String get nameSpeakers => 'Назвать говорящих';

  @override
  String get selectAMeetingToSeeIts =>
      'Выберите встречу, чтобы увидеть её расшифровку.';

  @override
  String get renameMeeting => 'Переименовать встречу';

  @override
  String get deleteMeeting => 'Удалить встречу';

  @override
  String recordedOn(Object devices) {
    return 'Записано на: $devices';
  }

  @override
  String get split => 'Разделить';

  @override
  String get longPressOrRightClickA =>
      'Нажмите и удерживайте фрагмент или щёлкните его правой кнопкой, чтобы исправить.';

  @override
  String get noTranscriptIsAvailable => 'Расшифровка недоступна.';

  @override
  String get theSpeakerIsAGuess => 'Говорящий определён предположительно';

  @override
  String get peopleTalkedAtOnceHereWords =>
      'Здесь говорили одновременно; слова могут быть пропущены';

  @override
  String get overlappingSpeech => 'наложение речи';

  @override
  String get someWordsWereUnclear => 'Некоторые слова были неразборчивы';

  @override
  String get addedAfterTheMeeting => 'добавлено после встречи';

  @override
  String photoOpenFullScreen(Object fileName) {
    return 'Фото $fileName, открыть на весь экран';
  }

  @override
  String get save => 'Сохранить';

  @override
  String get preview => 'Предпросмотр';

  @override
  String get editNotes => 'Изменить заметки';

  @override
  String get writeNotes => 'Написать заметки';

  @override
  String get loading => 'Загрузка…';

  @override
  String get noNotesYet => 'Заметок пока нет.';

  @override
  String textAt(Object startTimeMs) {
    return 'Текст в $startTimeMs';
  }

  @override
  String get removedThisTranscriptWonTBe =>
      'Удалено. Эта расшифровка не будет переделана после встречи.';

  @override
  String get savedThisTranscriptWonTBe =>
      'Сохранено. Эта расшифровка не будет переделана после встречи.';

  @override
  String get editText => 'Изменить текст';

  @override
  String get editTheTranscript => 'Изменить расшифровку';

  @override
  String get leaveItEmptyToRemoveThis =>
      'Оставьте пустым, чтобы удалить этот фрагмент.';

  @override
  String get searchMeetings => 'Поиск по встречам';

  @override
  String get chatWithAllYourNotes => 'Чат со всеми заметками';

  @override
  String get chatWithAllNotes => 'Чат со всеми заметками';

  @override
  String get refresh => 'Обновить';

  @override
  String get noMeetingsYet => 'Встреч пока нет';

  @override
  String get recordingsYouStartAppearHere =>
      'Здесь появятся начатые вами записи.';

  @override
  String get rename => 'Переименовать';

  @override
  String get addToProject => 'Добавить в проект…';

  @override
  String get summary2 => 'Резюме…';

  @override
  String get addToProject2 => 'Добавить в проект';

  @override
  String writingTheSummaryOf(Object title) {
    return 'Пишется резюме: $title';
  }

  @override
  String summaryOf(Object title) {
    return 'Резюме: $title';
  }

  @override
  String summarizeWith(Object label) {
    return 'Резюмировать с $label';
  }

  @override
  String defaultLabel2(Object model) {
    return '$model · по умолчанию';
  }

  @override
  String get summarizeOnMyComputer => 'Резюмировать на моём компьютере';

  @override
  String get withClaudeCodeCodexOrThe =>
      'С Claude Code, Codex или подключением, настроенным там';

  @override
  String get useYourOwnAiApp2 => 'Использовать своё ИИ-приложение…';

  @override
  String get claudeChatgptOrAnotherAppWith =>
      'Claude, ChatGPT или другое приложение под вашей учётной записью';

  @override
  String get openSummary => 'Открыть резюме';

  @override
  String get editPromptForThisMeeting => 'Изменить промпт для этой встречи…';

  @override
  String get useTheSettingsPrompt => 'Использовать промпт из настроек';

  @override
  String get promptForThisMeeting => 'Промпт для этой встречи';

  @override
  String placeholders(Object placeholders) {
    return 'Подстановки: $placeholders';
  }

  @override
  String get addAnAiConnectionInSettings3 =>
      'Добавьте ИИ-подключение в настройках, чтобы писать резюме.';

  @override
  String get summarizeWith2 => 'Резюмировать с помощью';

  @override
  String get summarize => 'Резюмировать';

  @override
  String get summarizeAgain => 'Резюмировать снова';

  @override
  String get editPromptForThisMeeting2 => 'Изменить промпт для этой встречи';

  @override
  String get writingTheSummaryALocalModel =>
      'Пишется резюме… Локальной модели может понадобиться несколько минут.';

  @override
  String get noSummaryYet => 'Резюме пока нет.';

  @override
  String get writeASummary => 'Написать резюме';

  @override
  String writtenByYouOn(Object createdAt) {
    return 'Написано вами $createdAt';
  }

  @override
  String writtenByOn(
      Object model, Object connectionLabel, Object createdAt, Object edited) {
    return 'Написано $model ($connectionLabel) $createdAt$edited';
  }

  @override
  String get editSummary => 'Изменить резюме';

  @override
  String get tryAgainOnMyComputer => 'Повторить на моём компьютере';

  @override
  String get cancelRequest => 'Отменить запрос';

  @override
  String get notesFolder => 'Папка заметок';

  @override
  String get keepYourProjectsAndMeetingsAs =>
      'Храните проекты и встречи как обычные файлы в папке, чтобы читать и редактировать их в любом редакторе. Изменения в notes.md, summary.md и файлах проекта попадают в приложение в течение 30 секунд, а файлы, положенные в папку проекта или папку вложений встречи, добавляются. Приложение перезаписывает transcript.md, поэтому исправляйте расшифровку в приложении.';

  @override
  String get theFolderIsADecryptedCopy =>
      'Папка — это расшифрованная копия ваших заметок: любой, кто может читать папку, может читать заметки, и инструменты резервного копирования или синхронизации тоже её копируют. Держите её в своей пользовательской папке на зашифрованном диске.';

  @override
  String get folder => 'Папка';

  @override
  String get chooseAFolder => 'Выбрать папку';

  @override
  String get keepTheNotesFolder => 'Вести папку заметок';

  @override
  String get syncNow => 'Синхронизировать сейчас';

  @override
  String lastSyncAtFailed(Object at, Object code) {
    return 'Последняя синхронизация в $at не удалась: $code';
  }

  @override
  String lastSyncAtFilesWrittenEdits(
      Object at, Object written, Object imported) {
    return 'Последняя синхронизация в $at: записано файлов — $written, принято изменений и файлов — $imported.';
  }

  @override
  String get inviteADevice => 'Пригласить устройство';

  @override
  String get pairingInviteQrCode => 'QR-код приглашения для сопряжения';

  @override
  String onTheOtherDeviceOpenDevices(Object hour, Object minute) {
    return 'На другом устройстве откройте «Устройства», выберите «Использовать приглашение» и вставьте эту ссылку. Можно также отсканировать код камерой и скопировать текст. Действует до $hour:$minute.';
  }

  @override
  String get close => 'Закрыть';

  @override
  String get inviteLinkCopied => 'Ссылка-приглашение скопирована';

  @override
  String get copyLink => 'Копировать ссылку';

  @override
  String get deviceFoundChoosePairUnderNearby =>
      'Устройство найдено. Выберите «Сопрячь» в разделе «Устройства поблизости».';

  @override
  String get useAnInvite => 'Использовать приглашение';

  @override
  String get inviteLink => 'Ссылка-приглашение';

  @override
  String get findDevice => 'Найти устройство';

  @override
  String get pairWithAnotherDevice =>
      'Выполнить сопряжение с другим устройством?';

  @override
  String get youOpenedAPairingInviteKoalino =>
      'Вы открыли приглашение для сопряжения. Koalino найдёт устройство, показавшее его, и оба устройства покажут код для сверки, прежде чем чем-либо делиться.';

  @override
  String get pairingStartedCheckThatBothDevices =>
      'Сопряжение начато. Проверьте, что оба устройства показывают один и тот же код.';

  @override
  String get done => 'Готово';

  @override
  String get photosAreAddedToTheMeeting =>
      'Фото добавляются к встрече по мере съёмки.';

  @override
  String get takePhoto => 'Сделать фото';

  @override
  String get theCameraIsUnavailable => 'Камера недоступна.';

  @override
  String search(Object name) {
    return 'Поиск: $name';
  }

  @override
  String get noteSaved => 'Заметка сохранена';

  @override
  String get theNoteCouldNotBeSaved => 'Не удалось сохранить заметку.';

  @override
  String couldNotBeAdded2(Object failed) {
    return 'Не удалось добавить: $failed.';
  }

  @override
  String get renameProject => 'Переименовать проект';

  @override
  String get theProjectCouldNotBeRenamed => 'Не удалось переименовать проект.';

  @override
  String delete3(Object project) {
    return 'Удалить «$project»?';
  }

  @override
  String get itsFilesNotesAndEMails =>
      'Файлы, заметки и письма будут удалены на всех сопряжённых устройствах. Встречи останутся без проекта.';

  @override
  String get theProjectCouldNotBeDeleted => 'Не удалось удалить проект.';

  @override
  String get itIsDeletedOnEveryPaired =>
      'Будет удалено на всех сопряжённых устройствах.';

  @override
  String get theFileCouldNotBeDeleted => 'Не удалось удалить файл.';

  @override
  String get theMeetingCouldNotBeRemoved =>
      'Не удалось убрать встречу из проекта.';

  @override
  String get project => 'Проект';

  @override
  String get searchThisProject => 'Поиск по проекту';

  @override
  String get chatAboutThisProject => 'Чат об этом проекте';

  @override
  String get addFiles => 'Добавить файлы';

  @override
  String get writeANote => 'Написать заметку';

  @override
  String get addMeetings => 'Добавить встречи';

  @override
  String get deleteProject => 'Удалить проект';

  @override
  String get addThemHereOrFromA => 'Добавьте их здесь или со страницы встречи.';

  @override
  String get removeFromProject => 'Убрать из проекта';

  @override
  String get filesNotesAndEMails => 'Файлы, заметки и письма';

  @override
  String get nothingAddedYet => 'Пока ничего не добавлено';

  @override
  String get addPicturesMarkdownPdfsEMails =>
      'Добавляйте изображения, Markdown, PDF, письма (.eml) и документы.';

  @override
  String get onItsWayFromAPaired => 'Передаётся с сопряжённого устройства';

  @override
  String get newNote => 'Новая заметка';

  @override
  String get noteMarkdown => 'Заметка (Markdown)';

  @override
  String get everyMeetingIsAlreadyInA =>
      'Все встречи уже находятся в проектах.';

  @override
  String get add => 'Добавить';

  @override
  String get moveToProject => 'Переместить в проект';

  @override
  String get noProject => 'Без проекта';

  @override
  String get newProject => 'Новый проект…';

  @override
  String get newProject2 => 'Новый проект';

  @override
  String get create => 'Создать';

  @override
  String get theProjectCouldNotBeCreated => 'Не удалось создать проект.';

  @override
  String get theMeetingCouldNotBeMoved => 'Не удалось переместить встречу.';

  @override
  String get removedFromItsProject => 'Убрано из проекта';

  @override
  String movedTo(Object project) {
    return 'Перемещено в $project';
  }

  @override
  String get searchProjects => 'Поиск по проектам';

  @override
  String get noProjectsYetAProjectGathers =>
      'Проектов пока нет. Проект объединяет встречи и связанные с ними файлы, заметки и письма.';

  @override
  String get theRecordingIsStillStartingTry =>
      'Запись ещё запускается. Попробуйте ещё раз.';

  @override
  String recording(Object title) {
    return 'Идёт запись «$title»';
  }

  @override
  String get recording2 => 'Идёт запись';

  @override
  String paused(Object title) {
    return '«$title» на паузе';
  }

  @override
  String get paused2 => 'Пауза';

  @override
  String get finishingTranscription => 'Завершение расшифровки…';

  @override
  String get meetingTitle => 'Название встречи';

  @override
  String get enterMeetingTitleOptional =>
      'Введите название встречи (необязательно)';

  @override
  String get leaveEmptyToNameTheMeeting =>
      'Оставьте пустым, чтобы назвать встречу по времени начала';

  @override
  String get startingRecording => 'Запуск записи';

  @override
  String get startRecording => 'Начать запись';

  @override
  String get pause => 'Пауза';

  @override
  String get resume => 'Продолжить';

  @override
  String get stop => 'Стоп';

  @override
  String get language => 'Язык';

  @override
  String get aChangeAppliesToWhatIs =>
      'Изменение применяется к тому, что будет сказано дальше';

  @override
  String get liveTranscript => 'Расшифровка в реальном времени';

  @override
  String get theTranscriptAppearsHereAFew =>
      'Расшифровка появляется здесь через несколько секунд после каждой паузы в речи.';

  @override
  String get nearbyDevicesSeeThisNameWhile =>
      'Устройства поблизости видят это имя, пока это устройство видимо. Сопряжённые устройства сохраняют уже известное им имя.';

  @override
  String get meetingRenamed => 'Встреча переименована';

  @override
  String get thisTitleIsnTAllowed => 'Такое название недопустимо.';

  @override
  String get couldnTSaveTheNewTitle =>
      'Не удалось сохранить новое название. Попробуйте ещё раз.';

  @override
  String get theNoteTakingEngineIsnT => 'Движок заметок сейчас недоступен.';

  @override
  String get thisMeetingNoLongerExists2 => 'Эта встреча больше не существует.';

  @override
  String get couldnTRenameTheMeeting => 'Не удалось переименовать встречу.';

  @override
  String get thisFileIsStillOnIts =>
      'Этот файл ещё передаётся с сопряжённого устройства.';

  @override
  String get thisPictureCannotBeShown => 'Это изображение нельзя показать.';

  @override
  String get thisKindOfDocumentCanT =>
      'Документы такого типа пока нельзя открыть в приложении.';

  @override
  String get thisFileCannotBeOpened => 'Этот файл нельзя открыть.';

  @override
  String get thisFileCannotBeRead => 'Этот файл нельзя прочитать.';

  @override
  String get thisEMailCannotBeRead => 'Это письмо нельзя прочитать.';

  @override
  String get noSubject => '(без темы)';

  @override
  String get from => 'От';

  @override
  String get to => 'Кому';

  @override
  String get cc => 'Копия';

  @override
  String get sent => 'Отправлено';

  @override
  String attachments(Object attachmentNames) {
    return 'Вложения: $attachmentNames';
  }

  @override
  String get theRestOfThisEMail =>
      'Остальная часть письма слишком длинная для показа.';

  @override
  String get searching => 'Поиск…';

  @override
  String get nothingFound => 'Ничего не найдено.';

  @override
  String get search2 => 'Поиск';

  @override
  String get pairYourDevicesAndSeeWhich =>
      'Сопрягайте устройства и смотрите, какие из них подключены.';

  @override
  String get transcriptionModel => 'Модель расшифровки';

  @override
  String get chooseTheWhisperModel => 'Выберите модель Whisper.';

  @override
  String get languageModelsForMeetingSummariesAnd =>
      'Языковые модели для резюме встреч и чата и их промпты.';

  @override
  String get keepProjectsAndMeetingsAsFiles =>
      'Храните проекты и встречи как файлы, которые можно редактировать в любом редакторе.';

  @override
  String get letYourOwnAssistantSuchAs =>
      'Разрешите своему ассистенту, например Claude Code, читать ваши заметки.';

  @override
  String get appLock => 'Блокировка приложения';

  @override
  String get defaultLanguage => 'Язык по умолчанию';

  @override
  String get usedForNewRecordingsAutoDetects =>
      'Используется для новых записей. «Автоматически» определяет язык речи.';

  @override
  String get transcribeOn => 'Где расшифровывать';

  @override
  String get anotherDeviceReceivesTheRecordingS =>
      'Другое устройство получает аудио записи в зашифрованном виде и удаляет его после расшифровки. Если оно недоступно, расшифровывает это устройство.';

  @override
  String get telemetry => 'Телеметрия';

  @override
  String get howLongTranscriptionTakesOnEach =>
      'Сколько длится расшифровка на каждом устройстве и оценка энергопотребления.';

  @override
  String get logLevel => 'Уровень журнала';

  @override
  String get howMuchDetailTheAppWrites =>
      'Насколько подробно приложение пишет журнал. «Отладка» и «Трассировка» помогают диагностировать проблемы, но пишут больше.';

  @override
  String get custom => 'Своё';

  @override
  String get shareLogs => 'Поделиться журналами';

  @override
  String get theLast7DaysOfDiagnostic =>
      'Диагностические журналы за последние 7 дней, например для отчёта об ошибке.';

  @override
  String get unableToReadTheAppLock =>
      'Не удалось прочитать настройку блокировки приложения.';

  @override
  String get needsAndroid10OrLater => 'Требуется Android 10 или новее.';

  @override
  String get setAScreenLockInAndroid =>
      'Настройте блокировку экрана в настройках Android, чтобы использовать блокировку приложения.';

  @override
  String askForYourFingerprintFaceOr(Object lockAfterBackground) {
    return 'Запрашивать отпечаток пальца, лицо или блокировку экрана при возврате в Koalino после отсутствия более $lockAfterBackground секунд. Запись и синхронизация продолжаются, пока приложение заблокировано.';
  }

  @override
  String get otherNetworks => 'Другие сети';

  @override
  String get aRelayOnYourOwnAlways =>
      'Ретранслятор на вашем постоянно включённом устройстве позволяет устройствам связываться, даже когда они не в одной сети Wi-Fi. Синхронизация в одной сети работает и без него.';

  @override
  String get relayAddress => 'Адрес ретранслятора';

  @override
  String get deviceAddressesOptionalOnePerLine =>
      'Адреса устройств (необязательно, по одному на строку)';

  @override
  String get saveAddresses => 'Сохранить адреса';

  @override
  String get nothingThatWasSharedCouldBe =>
      'Не удалось прочитать ничего из того, чем поделились.';

  @override
  String couldNotBeRead(Object message, Object failedCount) {
    return '$message; не удалось прочитать: $failedCount';
  }

  @override
  String saveAsTheSummaryOf(Object title) {
    return 'Сохранить как резюме «$title»';
  }

  @override
  String get addToAProjectInstead => 'Добавить в проект вместо этого';

  @override
  String get splitThisMeeting => 'Разделить эту встречу?';

  @override
  String itWasRecordedOnAndThe(Object devices, Object recordings) {
    return 'Встреча записана на $devices, и приложение объединило записи. После разделения другие записи ($recordings) появятся здесь и на сопряжённых устройствах как отдельные встречи со своей расшифровкой и больше не будут объединяться. Расшифровка этой встречи не изменится.';
  }

  @override
  String get unableToSplitThisMeeting => 'Не удалось разделить встречу.';

  @override
  String get telemetryDashboard => 'Панель телеметрии';

  @override
  String get transcription => 'Расшифровка';

  @override
  String get latency => 'Задержка';

  @override
  String get tasks => 'Задачи';

  @override
  String get power => 'Энергия';

  @override
  String get transcriptionDurationOverTime =>
      'Длительность расшифровки во времени';

  @override
  String get latencyDistribution => 'Распределение задержки';

  @override
  String get taskTypeBreakdown => 'Разбивка по типам задач';

  @override
  String get powerConsumptionEstimate => 'Оценка энергопотребления';

  @override
  String get fastestLeastAccurate => 'Самая быстрая, наименее точная';

  @override
  String get fastManyMistakesTheAppShips =>
      'Быстрая, много ошибок; входит в приложение';

  @override
  String get fastManyMistakes => 'Быстрая, много ошибок';

  @override
  String get goodBalanceOnMostComputers =>
      'Хороший баланс на большинстве компьютеров';

  @override
  String get accurateSlowerNeedsAbout2Gb =>
      'Точная, медленнее, нужно около 2 ГБ памяти';

  @override
  String get olderNeedsAPowerfulComputer => 'Старая, нужен мощный компьютер';

  @override
  String get mostAccurateNeedsAPowerfulComputer =>
      'Самая точная, нужен мощный компьютер';

  @override
  String get veryAccurateBestWithAGraphics =>
      'Очень точная, лучше всего с видеокартой';

  @override
  String get thisModelMayBeTooLarge => 'Эта модель может быть слишком большой';

  @override
  String mayNeedMoreMemoryThanThis(Object model) {
    return '$model может потребовать больше памяти, чем есть у этого устройства, и записи могут завершаться ошибкой или идти медленно.';
  }

  @override
  String get downloadAnyway => 'Всё равно загрузить';

  @override
  String get transcriptionSettingsAreUnavailable =>
      'Настройки расшифровки недоступны.';

  @override
  String get downloadFromQualcomm => 'Загрузить с Qualcomm?';

  @override
  String get about360MbIsDownloadedFrom =>
      'С Qualcomm загружается около 360 МБ, на устройстве занимается около 460 МБ:\n\n• Среда выполнения Qualcomm QNN, которая позволяет приложению использовать NPU. Она распространяется по лицензии Qualcomm AI Stack License, а не по лицензии приложения.\n• Whisper Small, скомпилированная Qualcomm AI Hub для NPU этого телефона.\n\nКаждый файл проверяется перед использованием. Записи никогда не покидают телефон. Загружая, вы принимаете условия Qualcomm.';

  @override
  String get acceptAndDownload => 'Принять и загрузить';

  @override
  String get downloadingTheNpuFiles => 'Загрузка файлов для NPU…';

  @override
  String get transcribeOnTheNpu => 'Расшифровывать на NPU';

  @override
  String get thisPhoneHasAnAiChip =>
      'В этом телефоне есть ИИ-чип (NPU), способный запускать Whisper Small, не нагружая процессор: около 2 секунд на 30 секунд речи. Требуется загрузка с Qualcomm.';

  @override
  String get download => 'Загрузить';

  @override
  String get runsWhisperSmallOnThePhone =>
      'Запускает Whisper Small на ИИ-чипе телефона вместо выбранной ниже модели, а также для обработки после встречи, если для неё нет своей модели. Время слов приблизительное.';

  @override
  String get removeTheNpuFiles => 'Удалить файлы для NPU';

  @override
  String get alsoRecordTheComputerSSound => 'Также записывать звук компьютера';

  @override
  String get transcribesWhatTheComputerPlaysSuch =>
      'Расшифровывает то, что воспроизводит компьютер, например голоса собеседников, вместе с микрофоном. Если микрофон тоже слышит динамики, повторённые слова сохраняются один раз.';

  @override
  String get improveTheTranscriptAfterTheMeeting =>
      'Улучшать расшифровку после встречи';

  @override
  String get transcribesEachMeetingAgainWithMore =>
      'После окончания встречи расшифровывает её заново с учётом большего контекста. Телефон ждёт подключения к зарядке. До этого аудио хранится в зашифрованном виде, затем удаляется.';

  @override
  String get modelForThisPass => 'Модель для этой обработки';

  @override
  String get aLargerModelIsSlowerBut =>
      'Большая модель медленнее, но точнее, а после встречи время есть. Загрузите модели выше.';

  @override
  String get sameAsForRecording => 'Та же, что и для записи';

  @override
  String get keepSpeechForCombining => 'Хранить речь для объединения';

  @override
  String get whenAMeetingWasAlsoRecorded =>
      'Если встреча записывалась и на другом устройстве, её аудио ждёт объединения столько времени, а затем удаляется.';

  @override
  String get whisperModel => 'Модель Whisper';

  @override
  String get biggerModelsMakeFewerMistakesAnd =>
      'Большие модели ошибаются реже, но требуют больше времени и памяти. Загрузка с Hugging Face. Выбор применяется только к этому устройству.';

  @override
  String automatic2(Object fileName, Object mode) {
    return 'Автоматически: $fileName$mode';
  }

  @override
  String usesUntilIsDownloadedTheBest(
      Object fileName, Object waiting, Object reason) {
    return 'Использует $fileName, пока не загрузится $waiting — лучшая модель для этого устройства. $reason';
  }

  @override
  String picksTheBestModelForThis(Object fileName, Object mode) {
    return 'Выбирает лучшую модель для этого устройства: $fileName$mode.';
  }

  @override
  String download2(Object sizeBytes) {
    return 'Загрузить $sizeBytes';
  }

  @override
  String tryLabel(Object model) {
    return 'Попробовать $model';
  }

  @override
  String get use => 'Использовать';

  @override
  String suggestedForThisDevice(Object model, Object mode) {
    return 'Рекомендуется для этого устройства: $model$mode';
  }

  @override
  String get efficientMode => 'Экономичный режим';

  @override
  String runsTheSameModelsOnThe(Object gpuName) {
    return 'Запускает те же модели на графическом чипе ($gpuName), обычно быстрее. Выключите, чтобы использовать процессор как обычно.';
  }

  @override
  String get theGraphicsChipStoppedTheApp =>
      'В прошлый раз графический чип остановил приложение, поэтому это устройство расшифровывает на процессоре.';

  @override
  String get tryTheGraphicsChipAgain => 'Снова попробовать графический чип';

  @override
  String get onThisDevice => 'На этом устройстве';

  @override
  String get usedByAutomatic => 'Используется в режиме «Автоматически»';

  @override
  String get englishOnly => 'Только английский';

  @override
  String get mayBeTooLargeForThis =>
      'Может быть слишком большой для этого устройства';

  @override
  String get doesnTMatchYourLanguage => 'Не соответствует вашему языку';

  @override
  String get notEnoughFreeSpace => 'Недостаточно свободного места';

  @override
  String get cancelDownload => 'Отменить загрузку';

  @override
  String get glossary => 'Глоссарий';

  @override
  String get namesProjectNamesAndTermsOne =>
      'Имена, названия проектов и термины, по одному на строку. Расшифровка пишет их так, как здесь.';

  @override
  String get saveGlossary => 'Сохранить глоссарий';

  @override
  String get voiceGallery => 'Галерея голосов';

  @override
  String get tryAgain => 'Повторить';

  @override
  String get unknownVoices => 'Неизвестные голоса';

  @override
  String get noUnknownSpeakersToReview =>
      'Нет неизвестных говорящих для проверки';

  @override
  String get savedPeople => 'Сохранённые люди';

  @override
  String get noSavedPeopleYetNameA =>
      'Сохранённых людей пока нет. Назовите голос, чтобы добавить его.';

  @override
  String delete4(Object displayName) {
    return 'Удалить $displayName';
  }

  @override
  String get playing => 'Воспроизведение…';

  @override
  String get untitledMeeting => 'Встреча без названия';

  @override
  String get play => 'Воспроизвести';

  @override
  String get deleteThisVoice => 'Удалить этот голос';

  @override
  String theCallShowedSpeakingCheckThen(Object suggestedName) {
    return 'Звонок показывал, что говорит $suggestedName. Проверьте, затем нажмите «Назначить».';
  }

  @override
  String get chooseASavedPerson => 'Выберите сохранённого человека';

  @override
  String get assign => 'Назначить';

  @override
  String get deleteTheseVoices => 'Удалить эти голоса';

  @override
  String get theseSoundLikeOnePersonUntick =>
      'Похоже, это один человек. Снимите отметку с чужих голосов.';

  @override
  String itsVoiceSamplesGoForGood(Object voices) {
    return 'Образцы голоса ($voices) будут удалены навсегда — здесь и на сопряжённых устройствах. Расшифровки останутся.';
  }

  @override
  String get itsVoiceSampleGoesForGood =>
      'Образец голоса будет удалён навсегда — здесь и на сопряжённых устройствах. Расшифровки останутся.';

  @override
  String get theirNameAndVoiceSamplesGo =>
      'Имя и образцы голоса будут удалены навсегда — здесь и на сопряжённых устройствах. Расшифровки останутся и будут показывать пронумерованного говорящего.';

  @override
  String unknownSpeaker(Object index) {
    return 'Неизвестный говорящий $index';
  }

  @override
  String attachedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Прикреплено $count файла',
      many: 'Прикреплено $count файлов',
      few: 'Прикреплено $count файла',
      one: 'Прикреплён $count файл',
    );
    return '$_temp0';
  }

  @override
  String addedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Добавлено $count файла',
      many: 'Добавлено $count файлов',
      few: 'Добавлено $count файла',
      one: 'Добавлен $count файл',
    );
    return '$_temp0';
  }

  @override
  String emailAttachmentsLeft(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count вложения письма остались в письме',
      many: '$count вложений письма остались в письме',
      few: '$count вложения письма остались в письме',
      one: '$count вложение письма осталось в письме',
    );
    return '$_temp0';
  }

  @override
  String addedMeetings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Добавлено $count встречи',
      many: 'Добавлено $count встреч',
      few: 'Добавлено $count встречи',
      one: 'Добавлена $count встреча',
    );
    return '$_temp0';
  }

  @override
  String otherRecordings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count записи',
      many: '$count записей',
      few: '$count записи',
      one: '$count запись',
    );
    return '$_temp0';
  }

  @override
  String get severalDevices => 'нескольких устройствах';

  @override
  String meetingCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count встречи',
      many: '$count встреч',
      few: '$count встречи',
      one: '$count встреча',
    );
    return '$_temp0';
  }

  @override
  String fileCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count файла',
      many: '$count файлов',
      few: '$count файла',
      one: '$count файл',
    );
    return '$_temp0';
  }

  @override
  String chunkCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count фрагмента',
      many: '$count фрагментов',
      few: '$count фрагмента',
      one: '$count фрагмент',
    );
    return '$_temp0';
  }

  @override
  String resultCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count результата',
      many: '$count результатов',
      few: '$count результата',
      one: '$count результат',
    );
    return '$_temp0';
  }

  @override
  String get moreNotShown =>
      '; есть ещё, не показанные здесь — уточните запрос';

  @override
  String cpuCores(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count ядра CPU',
      many: '$count ядер CPU',
      few: '$count ядра CPU',
      one: '$count ядро CPU',
    );
    return '$_temp0';
  }

  @override
  String dayCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count дня',
      many: '$count дней',
      few: '$count дня',
      one: '$count день',
    );
    return '$_temp0';
  }

  @override
  String voiceSampleCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count образца голоса',
      many: '$count образцов голоса',
      few: '$count образца голоса',
      one: '$count образец голоса',
      zero: 'пока нет образца голоса, поэтому не распознаётся по голосу',
    );
    return '$_temp0';
  }

  @override
  String picturesIncluded(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'включено $count изображения',
      many: 'включено $count изображений',
      few: 'включено $count изображения',
      one: 'включено $count изображение',
    );
    return '$_temp0';
  }

  @override
  String partCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count части',
      many: '$count частей',
      few: '$count части',
      one: '$count часть',
    );
    return '$_temp0';
  }

  @override
  String inMeetings(Object parts, Object meetings) {
    return '$parts в $meetings';
  }

  @override
  String assignedVoicesTo(int count, Object name) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count голоса назначены: $name',
      many: '$count голосов назначены: $name',
      few: '$count голоса назначены: $name',
      one: 'Назначено: $name',
    );
    return '$_temp0';
  }

  @override
  String addSharedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Добавить $count общих файла в проект',
      many: 'Добавить $count общих файлов в проект',
      few: 'Добавить $count общих файла в проект',
      one: 'Добавить общий файл в проект',
    );
    return '$_temp0';
  }

  @override
  String keepSpeechSaved(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Сохранено. Речь ждёт объединения до $count дня.',
      many: 'Сохранено. Речь ждёт объединения до $count дней.',
      few: 'Сохранено. Речь ждёт объединения до $count дней.',
      one: 'Сохранено. Речь ждёт объединения до $count дня.',
    );
    return '$_temp0';
  }

  @override
  String titlesCanBeAtMost(Object max) {
    return 'Название может содержать не более $max символов.';
  }

  @override
  String promptUnknownPlaceholder(Object example) {
    return 'В промпте есть неизвестная или незакрытая подстановка $example.';
  }

  @override
  String get live => '● В сети';

  @override
  String get offline => '○ Не в сети';

  @override
  String minutesShort(Object minutes) {
    return '$minutes мин';
  }

  @override
  String get appLanguage => 'Язык приложения';

  @override
  String get appLanguageDescription => 'Язык меню и сообщений Koalino.';

  @override
  String get systemLanguage => 'Как в системе';

  @override
  String photosAddedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Добавлено $count фото',
      many: 'Добавлено $count фото',
      few: 'Добавлено $count фото',
      one: 'Добавлено $count фото',
    );
    return '$_temp0';
  }

  @override
  String moreModels(int count) {
    return 'Другие модели ($count)';
  }

  @override
  String get moreModelsHint =>
      'Облегчённые версии, которые ошибаются чуть чаще, версии только для английского и модели, которые могут не поместиться на этом устройстве.';

  @override
  String get microphoneSilencedByCall =>
      'Android заглушает микрофон, пока другое приложение использует его для звонка: до конца звонка ничего не записывается.';

  @override
  String get handoffInTheBrowser =>
      'Koalino откроет сайт в браузере, где вы вошли в свою учётную запись, и передаст ему встречу с вашим запросом. Следующее окно покажет, что там делать.';

  @override
  String toKeepTheSummaryCopyTheWebsite(String appName) {
    return 'Чтобы сохранить резюме, скопируйте ответ $appName на сайте и вернитесь в Koalino.';
  }

  @override
  String get useYourClaudeOrChatgptPlan =>
      'Использовать ваш тариф Claude или ChatGPT';

  @override
  String get summariesCanUseYourOwnPlan =>
      'Резюме может создаваться по вашей подписке Claude или ChatGPT без ключа API. Установите Claude Code или Codex и войдите один раз: дальше Koalino пишет резюме сам.';

  @override
  String get installClaudeCode => 'Установить Claude Code';

  @override
  String get forAClaudePlanOpensTheGuide =>
      'Для тарифа Claude. Открывает инструкцию по установке.';

  @override
  String get installCodex => 'Установить Codex';

  @override
  String get forAChatgptPlanOpensTheGuide =>
      'Для тарифа ChatGPT. Открывает инструкцию по установке.';

  @override
  String get skipUseTheWebsite => 'Пропустить: использовать сайт';

  @override
  String get koalinoCopiesTheMeetingAndOpens =>
      'Ничего не нужно устанавливать: Koalino копирует встречу и открывает claude.ai или chatgpt.com, а вы вставляете её туда.';

  @override
  String get addAConnection => 'Добавить подключение';

  @override
  String get claudeCodeOrCodexAlreadyInstalled =>
      'Claude Code или Codex уже установлены, или сервер в вашей сети.';

  @override
  String get noConnectionNeededUseTheWebsite =>
      'Подключение не нужно. Во встрече откройте «Резюме» и выберите «Использовать своё ИИ-приложение…»: Koalino скопирует встречу и откроет claude.ai или chatgpt.com.';

  @override
  String get afterInstallingAddTheConnection =>
      'После установки и входа вернитесь и выберите «Добавить подключение» → «Добавить подключение» → Claude Code или Codex.';

  @override
  String get claudeAiOrChatgptComInYourBrowser =>
      'claude.ai или chatgpt.com в браузере, под вашей учётной записью';

  @override
  String get newMeeting => 'Новая встреча';

  @override
  String get newMeetingExplanation =>
      'Встреча без записи. Напишите заметки или добавьте файлы: их используют сводки и чат.';

  @override
  String get titleOptional => 'Название (необязательно)';

  @override
  String handoffStepPressEnter(String site) {
    return '$site откроется со встречей уже в поле сообщения. Нажмите Enter, чтобы отправить её.';
  }

  @override
  String handoffStepSentByItself(String site) {
    return '$site откроется и сам отправит встречу. Если попросит, сначала войдите.';
  }

  @override
  String handoffStepPaste(String site) {
    return '$site откроется, а Koalino скопирует встречу. Щёлкните в поле сообщения, вставьте её (Ctrl+V) и нажмите Enter.';
  }

  @override
  String get handoffStepDragFiles =>
      'Также откроется папка с файлами: перетащите их все в поле сообщения, чтобы они ушли как файлы.';

  @override
  String get handoffStepCopyAnswer =>
      'Когда ответ готов, нажмите кнопку копирования под ним.';

  @override
  String get handoffStepComeBack =>
      'Вернитесь в Koalino: ответ станет сводкой встречи, с возможностью отменить.';

  @override
  String get handoffStepAskThere => 'Затем задавайте там вопросы о встрече.';

  @override
  String openSite(String site) {
    return 'Открыть $site';
  }

  @override
  String writtenBySourceOn(String source, String createdAt) {
    return 'Написано: $source, $createdAt';
  }

  @override
  String get noRecordingWriteNotesOrAttach =>
      'Записи нет. Напишите заметки или прикрепите файлы: их используют сводки и чат.';

  @override
  String handoffStepOpensFilled(String site) {
    return '$site откроется со встречей уже в поле сообщения.';
  }

  @override
  String handoffStepOpensCopied(String site) {
    return '$site откроется, а Koalino скопирует встречу.';
  }

  @override
  String get handoffStepSend => 'Нажмите Enter, чтобы отправить.';

  @override
  String get handoffStepPasteSend =>
      'Щёлкните в поле сообщения, вставьте встречу (Ctrl+V) и нажмите Enter.';

  @override
  String get chatHandoffIntro =>
      'Задавайте вопросы в своём ИИ-приложении, со своей учётной записью. Koalino передаёт ему заметки и расшифровки, а фото и файлы идут вместе с ними как файлы.';

  @override
  String get plainTextFile => 'Обычный текст (.txt)';

  @override
  String get unableToExportTheSummary => 'Не удалось экспортировать сводку';

  @override
  String get addAVoice => 'Добавить голос';

  @override
  String alreadySavedVoiceAddsToThem(String name) {
    return '$name уже сохранён: этот голос будет добавлен ему.';
  }

  @override
  String addVoicesTo(String name) {
    return 'Добавить голоса: $name';
  }

  @override
  String tickTheVoicesThatAre(String name) {
    return 'Эти голоса ни с кем не сопоставлены. Отметьте голоса $name; кнопка «Воспроизвести» поможет проверить.';
  }

  @override
  String addTo(String name) {
    return 'Добавить: $name';
  }

  @override
  String get apiKey => 'API-ключ';

  @override
  String get onlyForOnlineServicesKeptIn =>
      'Только для онлайн-сервисов. Хранится в связке ключей этого устройства и отправляется только на этот адрес.';

  @override
  String get aKeyIsSavedLeaveEmpty =>
      'Ключ сохранён. Оставьте пустым, чтобы сохранить его.';

  @override
  String get removeTheSavedKey => 'Удалить сохранённый ключ';

  @override
  String get presetOllama =>
      'На вашем компьютере или в сети: бесплатно, приватно, работает офлайн (Gemma, Qwen, Llama…).';

  @override
  String get presetLmStudio =>
      'Как Ollama, с настольным приложением для выбора моделей.';

  @override
  String get presetGemini =>
      'Google. Бесплатный тариф с ограничениями; на нём Google может использовать отправленное для улучшения продуктов. Ключ: aistudio.google.com.';

  @override
  String get presetDeepseek =>
      'Оплата за использование, очень дёшево, предоплата. Ключ: platform.deepseek.com.';

  @override
  String get presetQwen =>
      'Alibaba. Оплата за использование. Ключ: Alibaba Cloud Model Studio.';

  @override
  String get presetKimi =>
      'Moonshot AI. Оплата за использование. Ключ: platform.moonshot.ai.';

  @override
  String get presetMistral =>
      'Mistral (Франция). Бесплатный тариф со строгими лимитами. Ключ: console.mistral.ai.';

  @override
  String get presetOpenrouter =>
      'Один ключ для многих моделей (Gemini, DeepSeek, Qwen…), часть бесплатно; текст также проходит через OpenRouter. Ключ: openrouter.ai.';

  @override
  String get keepMeetingRecordings => 'Сохранять записи встреч';

  @override
  String get keepMeetingRecordingsSubtitle =>
      'Сохранять звук каждой встречи после расшифровки, в зашифрованном виде на этом устройстве, чтобы его можно было прослушать. По умолчанию выключено.';

  @override
  String get keepMeetingRecordingsQuestion => 'Сохранять записи встреч?';

  @override
  String get keepRecordingsWarning =>
      'Записи содержат голоса всех участников. Предупреждайте людей, которых записываете, и соблюдайте местные правила о согласии: во многих странах для записи других нужно их разрешение. Записи хранятся зашифрованными на этом устройстве и не синхронизируются; они занимают около 2 МБ на каждые 10 минут речи. Они удаляются вместе со встречей или все сразу кнопкой «Удалить все записи». Сохраняется только речь, поэтому паузы воспроизводятся как тишина.';

  @override
  String get keepRecordings => 'Сохранять записи';

  @override
  String recordingsKept(int count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count записи · $size МБ',
      many: '$count записей · $size МБ',
      few: '$count записи · $size МБ',
      one: '1 запись · $size МБ',
      zero: 'Записей нет',
    );
    return '$_temp0';
  }

  @override
  String get wipeAllRecordings => 'Удалить все записи';

  @override
  String get wipeAllRecordingsQuestion => 'Удалить все записи?';

  @override
  String get wipeAllRecordingsExplanation =>
      'Все сохранённые записи встреч на этом устройстве будут удалены. Расшифровки и заметки останутся. Это нельзя отменить.';

  @override
  String recordingsWiped(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Удалено $count записи.',
      many: 'Удалено $count записей.',
      few: 'Удалено $count записи.',
      one: 'Удалена 1 запись.',
      zero: 'Нечего удалять.',
    );
    return '$_temp0';
  }

  @override
  String get recordingsUnavailable => 'Записи сейчас недоступны.';

  @override
  String get playRecording => 'Прослушать запись';

  @override
  String get stopRecordingPlayback => 'Остановить воспроизведение';

  @override
  String get recordingCouldNotBePlayed => 'Не удалось воспроизвести запись.';

  @override
  String get columnWidth => 'Ширина столбца';

  @override
  String get subprojects => 'Подпроекты';

  @override
  String get newSubproject => 'Новый подпроект';

  @override
  String get moveProjectTo => 'Переместить в…';

  @override
  String get topLevel => 'Верхний уровень';

  @override
  String moveProjectTitle(String project) {
    return 'Переместить «$project» в';
  }

  @override
  String get movedToTopLevel => 'Перемещено на верхний уровень';

  @override
  String get theProjectCouldNotBeMoved => 'Не удалось переместить проект.';

  @override
  String subprojectCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count подпроекта',
      many: '$count подпроектов',
      few: '$count подпроекта',
      one: '1 подпроект',
    );
    return '$_temp0';
  }

  @override
  String get chooseAProject =>
      'Выберите проект, чтобы увидеть его встречи и файлы.';

  @override
  String get chooseAMeetingOrFile =>
      'Выберите встречу или файл, чтобы открыть его здесь.';

  @override
  String get backgroundPassTitle => 'Улучшать расшифровки при закрытом Koalino';

  @override
  String get backgroundPassAllowed =>
      'Разрешено: телефон улучшает ожидающие расшифровки во время зарядки, даже если Koalino закрыт или включено энергосбережение.';

  @override
  String get backgroundPassAsk =>
      'Нажмите, чтобы Android делал это во время зарядки, даже в режиме энергосбережения. Иначе это происходит только при открытом Koalino.';

  @override
  String get chatPrompt => 'Промпт чата';

  @override
  String chatPromptHelp(String placeholder) {
    return '$placeholder обозначает то, о чём вы спрашиваете: встречу, проект или все ваши заметки. Ответы всегда ссылаются на источники по номеру, а ваши заметки идут после промпта.';
  }

  @override
  String get saveChatPrompt => 'Сохранить промпт чата';

  @override
  String get chatPromptSaved => 'Промпт чата сохранён.';

  @override
  String get theBuiltInChatPromptIsBack =>
      'Встроенный промпт чата восстановлен.';

  @override
  String get promptsSharedWithPairedDevices =>
      'Оба промпта одинаковы на всех ваших сопряжённых устройствах: изменение здесь дойдёт до них, когда они подключатся.';

  @override
  String get deviceOnline => 'В сети';

  @override
  String get deviceOffline => 'Не в сети';

  @override
  String get neverSynced => 'Ещё не синхронизировано';

  @override
  String get syncedJustNow => 'Синхронизировано только что';

  @override
  String lastSyncedMinutesAgo(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Синхронизировано $count минуты назад',
      many: 'Синхронизировано $count минут назад',
      few: 'Синхронизировано $count минуты назад',
      one: 'Синхронизировано $count минуту назад',
    );
    return '$_temp0';
  }

  @override
  String lastSyncedHoursAgo(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Синхронизировано $count часа назад',
      many: 'Синхронизировано $count часов назад',
      few: 'Синхронизировано $count часа назад',
      one: 'Синхронизировано $count час назад',
    );
    return '$_temp0';
  }

  @override
  String lastSyncedOn(String date) {
    return 'Последняя синхронизация: $date';
  }

  @override
  String devicesOnlineCount(int online, int total) {
    return 'В сети: $online из $total';
  }

  @override
  String get microphoneSeemsMutedOrOff =>
      'Похоже, микрофон выключен или в нём отключён звук: ничего не слышно. Проверьте кнопку или клавишу отключения звука и то, что доступ к микрофону включён в настройках системы.';

  @override
  String get windowsBlocksTheMicrophone =>
      'Windows не даёт приложениям доступ к микрофону: ничего не слышно. В разделе Параметры → Конфиденциальность и защита → Микрофон включите «Доступ к микрофону» и «Разрешить классическим приложениям доступ к микрофону».';

  @override
  String get backUpAllData => 'Резервная копия всех данных';

  @override
  String get backUpAllDataDescription =>
      'Сохранить встречи, заметки, вложения и сохранённые записи в одном зашифрованном файле';

  @override
  String get restoreFromBackup => 'Восстановить из резервной копии';

  @override
  String get restoreFromBackupDescription =>
      'Заменить данные на этом устройстве резервной копией';

  @override
  String get backupPassphraseTitle => 'Парольная фраза копии';

  @override
  String get backupPassphraseExplanation =>
      'Копия шифруется этой парольной фразой. Без неё никто не сможет открыть копию, даже вы, поэтому храните её в надёжном месте.';

  @override
  String get backupPassphraseLabel => 'Парольная фраза';

  @override
  String get repeatPassphraseLabel => 'Повторите парольную фразу';

  @override
  String get passphraseTooShort => 'Используйте не менее 8 символов.';

  @override
  String get passphrasesDoNotMatch => 'Парольные фразы не совпадают.';

  @override
  String get continueAction => 'Продолжить';

  @override
  String get backingUp => 'Создание копии…';

  @override
  String get restoringBackup => 'Восстановление…';

  @override
  String backupSaved(int files, String size) {
    return 'Копия сохранена: файлов — $files, $size МБ.';
  }

  @override
  String get restoreConfirmTitle => 'Заменить данные на этом устройстве?';

  @override
  String get restoreConfirmBody =>
      'Встречи и заметки на этом устройстве будут заменены данными из копии. Текущие данные сохраняются отдельно до следующего восстановления. Затем Koalino перезапустится.';

  @override
  String get restoreAction => 'Восстановить';

  @override
  String restoreReady(int files) {
    return 'Восстановлено файлов: $files. Koalino перезапустится, чтобы открыть их.';
  }

  @override
  String get restartNow => 'Перезапустить';

  @override
  String get backupWrongPassphrase => 'Неверная парольная фраза.';

  @override
  String get backupNotABackup =>
      'Этот файл не является резервной копией Koalino.';

  @override
  String get backupTooNew =>
      'Эта копия создана более новой версией Koalino. Сначала обновите приложение.';

  @override
  String get backupDamaged => 'Копия повреждена или неполна.';

  @override
  String get backupFailed =>
      'Не удалось завершить операцию. Проверьте свободное место и повторите попытку.';

  @override
  String get automaticBackups => 'Автоматические копии';

  @override
  String get automaticBackupsDescription =>
      'Каждый день во время зарядки в выбранную папку, в том числе в облаке (Google Drive, Nextcloud, OneDrive…). Шифруется до того, как покинет телефон.';

  @override
  String automaticBackupFolder(String folder) {
    return 'Папка: $folder';
  }

  @override
  String get noAutomaticBackupYet => 'Автоматических копий пока нет.';

  @override
  String lastAutomaticBackup(String time) {
    return 'Последняя копия: $time';
  }

  @override
  String lastAutomaticBackupFailed(String time) {
    return 'Копия от $time не создана. Проверьте папку.';
  }

  @override
  String get backUpNow => 'Создать копию сейчас';

  @override
  String get backupFolderUnavailable =>
      'Не удаётся записать в папку копий. Проверьте, что она существует и её приложение авторизовано, или выберите другую папку.';

  @override
  String get automaticBackupsNotSetUp =>
      'Автоматические копии нужно настроить заново: выключите и снова включите их.';

  @override
  String get renameSpeaker => 'Переименовать говорящего';

  @override
  String get nameThisVoice => 'Назвать этот голос';

  @override
  String get doubleClickOrLongPressToRename =>
      'Дважды щёлкните или нажмите и удерживайте, чтобы переименовать';

  @override
  String get theNewNameAppliesToEveryPassage =>
      'Новое имя применяется ко всем фрагментам этого голоса во всех ваших встречах и на связанных устройствах.';

  @override
  String anotherSavedPersonIsCalled(Object name) {
    return 'Другого сохранённого человека уже зовут «$name». Если это один и тот же человек, объедините их.';
  }

  @override
  String get useOneTo100CharactersOnOneLine =>
      'Используйте от 1 до 100 символов в одной строке.';

  @override
  String versionNumber(Object version) {
    return 'Версия $version';
  }

  @override
  String get developerSettings => 'Настройки разработчика';

  @override
  String get appVariablesForExpertsAndDevelopers =>
      'Переменные приложения для специалистов и разработчиков';

  @override
  String tapsLeftToTurnOnDeveloperSettings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'Ещё $count нажатий, чтобы включить настройки разработчика',
      few: 'Ещё $count нажатия, чтобы включить настройки разработчика',
      one: 'Ещё $count нажатие, чтобы включить настройки разработчика',
    );
    return '$_temp0';
  }

  @override
  String get developerSettingsAreOn => 'Настройки разработчика включены';

  @override
  String get developerSettingsWarning =>
      'Эти переменные меняют внутреннюю работу Koalino. Неверное значение может остановить расшифровку, синхронизацию или расширение браузера. Сохранённые значения применяются после перезапуска Koalino.';

  @override
  String get restartKoalinoToApply =>
      'Перезапустите Koalino, чтобы применить сохранённые значения.';

  @override
  String nowAndDefault(Object now, Object defaultValue) {
    return 'Сейчас: $now · По умолчанию: $defaultValue';
  }

  @override
  String get notSet => 'не задано';

  @override
  String savedForTheNextStart(Object value) {
    return 'Сохранено до следующего запуска: $value';
  }

  @override
  String get useDefault => 'По умолчанию';

  @override
  String get resetAll => 'Сбросить всё';

  @override
  String get turnOffDeveloperSettings => 'Выключить настройки разработчика';

  @override
  String get turnOffDeveloperSettingsExplanation =>
      'При следующем запуске все значения вернутся к значениям по умолчанию, а этот раздел снова будет скрыт.';

  @override
  String get turnOff => 'Выключить';

  @override
  String mergeIntoAnotherPerson(Object name) {
    return 'Объединить $name с другим человеком';
  }

  @override
  String mergeNameInto(Object name) {
    return 'Объединить $name с…';
  }

  @override
  String mergeWarning(Object source, Object target) {
    return 'Все фрагменты и образцы голоса $source перейдут к $target, а $source будет удалён, в том числе на связанных устройствах. Это нельзя отменить.';
  }

  @override
  String get merge => 'Объединить';

  @override
  String mergeWithName(Object name) {
    return 'Объединить с $name';
  }

  @override
  String mergedInto(Object source, Object target) {
    return '$source теперь объединён с $target';
  }

  @override
  String get thesePeopleCouldNotBeMerged =>
      'Не удалось объединить этих людей. Попробуйте ещё раз.';

  @override
  String get peopleCannotBeMergedWhileRecording =>
      'Нельзя объединять людей во время записи встречи.';

  @override
  String get notEnoughFreeSpaceToDownload =>
      'Пока недостаточно свободного места, чтобы его скачать.';

  @override
  String get downloadTheBestModelByItself =>
      'Скачивать лучшую модель самостоятельно';

  @override
  String get downloadTheBestModelByItselfHint =>
      'По Wi-Fi и при наличии места модель, которую «Автоматически» выбирает для этого устройства, скачивается при запуске. Пока что записи используют текущую модель.';

  @override
  String gettingAMoreAccurateModel(String name, String percent) {
    return 'Загружается более точная модель ($name) для этого устройства: $percent';
  }

  @override
  String get recordingsUseTheCurrentModelMeanwhile =>
      'Пока она не готова, записи используют текущую модель.';

  @override
  String get youCanRecordOnceItIsReady =>
      'Записывать можно будет, как только она будет готова.';

  @override
  String get npuUsedAtTheLastRecording => 'Использовался при последней записи.';

  @override
  String get npuFailedAtTheLastRecording =>
      'NPU этого телефона не прошёл проверку, поэтому расшифровывает процессор. Чтобы попробовать снова, удалите файлы NPU и скачайте их заново.';

  @override
  String get skipBack10Seconds => 'Назад на 10 секунд';

  @override
  String get skipForward10Seconds => 'Вперёд на 10 секунд';

  @override
  String get playbackSpeed => 'Скорость воспроизведения';

  @override
  String get followTranscript => 'Следить по расшифровке';

  @override
  String get playFromHere => 'Воспроизвести отсюда';

  @override
  String get openingTheRecording => 'Открываем запись…';

  @override
  String get recordingPlayer => 'Проигрыватель записи';

  @override
  String get shareTheRecording => 'Поделиться записью';

  @override
  String get saveTheRecording => 'Сохранить запись';

  @override
  String get unableToShareTheRecording => 'Не удалось поделиться записью';

  @override
  String get unableToSaveTheRecording => 'Не удалось сохранить запись';

  @override
  String todayAt(String time) {
    return 'Сегодня $time';
  }

  @override
  String yesterdayAt(String time) {
    return 'Вчера $time';
  }

  @override
  String get unnamedDevice => 'Устройство без имени';

  @override
  String get checkTheMicrophone =>
      'Проверьте, что микрофон подключён и Koalino может его использовать.';

  @override
  String get speechModelCouldNotLoad =>
      'Не удалось начать расшифровку: речевая модель отсутствует или не загрузилась. Выберите или скачайте модель.';

  @override
  String get processorCannotTranscribe =>
      'Процессор этого устройства не может выполнять расшифровку. В настройках расшифровки её может выполнять другое ваше устройство.';

  @override
  String get secureStorageLocked =>
      'Защищённое хранилище Koalino заблокировано. Разблокируйте его и повторите попытку.';

  @override
  String get startKeepsFailing =>
      'Повторите попытку. Если не получится, перезапустите Koalino.';

  @override
  String get openTranscriptionSettings => 'Открыть настройки расшифровки';

  @override
  String get people => 'Люди';

  @override
  String get recordingNow => 'Идёт запись';

  @override
  String get pausedNow => 'Пауза';

  @override
  String get recordAMeeting => 'Записать встречу';

  @override
  String get voicesAppearAfterRecording =>
      'После записи здесь появляются голоса, которые Koalino не узнал, чтобы вы могли их назвать.';
}
