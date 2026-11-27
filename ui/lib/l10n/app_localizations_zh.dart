// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Chinese (`zh`).
class AppLocalizationsZh extends AppLocalizations {
  AppLocalizationsZh([String locale = 'zh']) : super(locale);

  @override
  String get unableToCheckTheAppLock => '无法检查应用锁。请重试。';

  @override
  String get unableToShowTheUnlockPrompt => '无法显示解锁提示。请重试。';

  @override
  String get setAScreenLockOnThis => '请先在此设备上设置屏幕锁定。';

  @override
  String get unableToChangeTheAppLock => '无法更改应用锁。请重试。';

  @override
  String get aCallApp => '通话应用';

  @override
  String get call => '通话';

  @override
  String call2(Object app) {
    return '$app 通话';
  }

  @override
  String theCallEndedSoTheRecording(Object tiedApp) {
    return '$tiedApp 通话已结束，因此录音已停止。';
  }

  @override
  String get restartYourBrowserAndCallApps =>
      '请重启一次浏览器和通话应用，以便 Koalino 读取通话中的发言人。';

  @override
  String get unableToLoadDevices => '无法加载设备';

  @override
  String get thisDeviceIsNotOnA => '此设备尚未连接网络。请连接后重试。';

  @override
  String get thatDidNotWorkTryAgain => '操作未成功。请重试。';

  @override
  String get use1To64CharactersWithout => '请使用 1 到 64 个字符，不含换行。';

  @override
  String get theNameCouldNotBeSaved => '无法保存名称。请重试。';

  @override
  String get thisInviteHasExpiredMakeA => '此邀请已过期。请在另一台设备上新建一个。';

  @override
  String get thisInviteWasMadeOnThis => '此邀请是在本设备上创建的。请在另一台设备上使用。';

  @override
  String get thatDeviceIsAlreadyPaired => '该设备已配对。';

  @override
  String get thatIsNotAValidInvite => '这不是有效的邀请。请重新复制。';

  @override
  String get deviceSyncIsNotRunning => '设备同步未运行。';

  @override
  String get theOtherDeviceIsNoLonger => '另一台设备已不在配对中。请重试。';

  @override
  String get theOtherAppCouldNotBe => '无法打开另一个应用。';

  @override
  String get theOtherApp => '另一个应用';

  @override
  String app(Object displayName) {
    return '$displayName 应用';
  }

  @override
  String get unableToChangeTheLogLevel => '无法更改日志级别';

  @override
  String logsExportedTo(Object filePath) {
    return '日志已导出到 $filePath';
  }

  @override
  String get unableToShareTheLogs => '无法分享日志';

  @override
  String get noLogsToExportYet => '暂无可导出的日志';

  @override
  String get unableToExportTheLogs => '无法导出日志';

  @override
  String couldNotBeAttached(Object attached, Object failedPaths) {
    return '$attached；$failedPaths 个无法附加';
  }

  @override
  String get theFilesCouldNotBeAttached => '无法附加文件。';

  @override
  String get unableToShareTheExport => '无法分享导出内容';

  @override
  String exportedTo(Object filePath) {
    return '已导出到 $filePath';
  }

  @override
  String get exportSaved => '导出已保存';

  @override
  String get unableToSaveTheExport => '无法保存导出内容';

  @override
  String get thisMeetingNoLongerExists => '此会议已不存在';

  @override
  String get exportIsUnavailableUntilTheApp => '应用启动完成前无法导出';

  @override
  String get unableToExportTheMeeting => '无法导出会议';

  @override
  String get unableToLoadMeetings => '无法加载会议';

  @override
  String get unableToLoadThisMeeting => '无法加载此会议';

  @override
  String get enterATitle => '请输入标题。';

  @override
  String titlesCanBeAtMostCharacters(
      Object maxMeetingTitleCharacters, Object runes) {
    return '标题最多 $maxMeetingTitleCharacters 个字符（当前 $runes 个）。';
  }

  @override
  String get titlesCanTContainLineBreaks => '标题不能包含换行或控制字符。';

  @override
  String get notPairedYetMakeTheRelay => '尚未配对。请让中继可被发现，然后在“设备”的“附近的设备”中与其配对。';

  @override
  String get connectingToTheRelay => '正在连接中继…';

  @override
  String get connectedToTheRelayAskingIt => '已连接中继。正在请求其接受连接…';

  @override
  String get connectedYourOtherDevicesCanReach => '已连接。你的其他设备可通过中继访问此设备。';

  @override
  String get savedThisAppliesNowWithNo => '已保存。立即生效，无需重启。';

  @override
  String get thatRelayAddressIsNotValid =>
      '该中继地址无效。格式类似 /ip4/203.0.113.5/tcp/4001/p2p/12D3Koo…';

  @override
  String get aDeviceAddressIsNotValid => '有一个设备地址无效。每个地址都以 /p2p/ 加设备 ID 结尾。';

  @override
  String get unableToSaveTheAddresses => '无法保存地址。';

  @override
  String get thePhotoCouldNotBeSaved => '无法保存照片。';

  @override
  String get enterAName => '请输入名称。';

  @override
  String namesCanBeAtMostCharacters(Object maxMeetingTitleCharacters) {
    return '名称最多 $maxMeetingTitleCharacters 个字符。';
  }

  @override
  String get namesCanTContainLineBreaks => '名称不能包含换行或控制字符。';

  @override
  String get unableToLoadProjects => '无法加载项目';

  @override
  String get unableToLoadThisProject => '无法加载此项目';

  @override
  String get theFilesCouldNotBeAdded => '无法添加文件。';

  @override
  String couldNotBeAdded(Object failedPaths) {
    return '$failedPaths 个无法添加';
  }

  @override
  String get unableToStartRecording => '无法开始录音';

  @override
  String get unableToPauseRecording => '无法暂停录音';

  @override
  String get unableToResumeRecording => '无法继续录音';

  @override
  String get unableToStopRecording => '无法停止录音';

  @override
  String get recordingSaved => '录音已保存';

  @override
  String get microphoneAccessIsNeededToRecord => '录音需要麦克风权限';

  @override
  String get keepTheAppOpenAndroidDid => '请保持应用打开：Android 不允许在后台录音';

  @override
  String get unableToOpenTheMicrophone => '无法打开麦克风';

  @override
  String saved(Object label) {
    return '已保存 $label。';
  }

  @override
  String removed(Object label) {
    return '已移除 $label。';
  }

  @override
  String isNowTheDefault(Object id) {
    return '$id 现在是默认连接。';
  }

  @override
  String answered(Object id, Object reply) {
    return '$id 回复：$reply';
  }

  @override
  String get promptSaved => '提示词已保存。';

  @override
  String get theBuiltInPromptIsBack => '已恢复内置提示词。';

  @override
  String get finishedMeetingsWillBeSummarizedAutomatically => '已结束的会议将自动生成摘要。';

  @override
  String get automaticSummariesAreOff => '自动摘要已关闭。';

  @override
  String get thisComputerNoLongerWritesSummaries => '此电脑不再为你的其他设备生成摘要或回答聊天。';

  @override
  String nowWritesTheSummariesAndAnswers(Object id) {
    return '$id 现在为你的其他设备生成摘要并回答聊天。';
  }

  @override
  String get requestCancelled => '请求已取消。';

  @override
  String get thisMeetingUsesTheSettingsPrompt => '此会议重新使用“设置”中的提示词。';

  @override
  String get promptSavedForThisMeeting => '已为此会议保存提示词。';

  @override
  String get summarySaved => '摘要已保存。';

  @override
  String get theConnection => '该连接';

  @override
  String get summaryWritten => '摘要已生成。';

  @override
  String theTranscriptWasReadInParts(Object parts) {
    return '转写分 $parts 部分读取。';
  }

  @override
  String leftOut(Object sent, Object imagesSkipped) {
    return '$sent，$imagesSkipped 张未包含。';
  }

  @override
  String get theModelStoppedBeforeTheEnd => '模型在结尾前停止了。';

  @override
  String get waitingForYourComputerItWrites =>
      '正在等待你的电脑。当它开机、连接到此设备，并在“设置 → AI 连接”中设为回答时，就会生成摘要。';

  @override
  String get yourComputerIsWritingTheSummary => '你的电脑正在生成摘要…';

  @override
  String yourComputerCouldNotWriteThe(Object message) {
    return '你的电脑无法生成摘要：$message';
  }

  @override
  String get unableToRefreshTelemetry => '无法刷新遥测数据';

  @override
  String get automatic => '自动';

  @override
  String get transcriptionLanguage => '转写语言';

  @override
  String get transcriptionLanguages => '转写语言';

  @override
  String get theLanguageCouldNotBeChanged => '无法更改语言。';

  @override
  String get thisDevice => '此设备';

  @override
  String get bestAvailable => '最佳可用';

  @override
  String get aRemovedDevice => '已移除的设备';

  @override
  String get theTranscriptionDeviceCouldNotBe => '无法更改转写设备。请检查它是否仍已配对。';

  @override
  String get downloadFinishedYouCanNowChoose => '下载完成。现在可以选择此模型。';

  @override
  String get downloadCancelled => '下载已取消。';

  @override
  String get theDownloadStoppedNoConnectionTo => '下载已停止：无法连接服务器。当前模型仍可使用。';

  @override
  String get theDownloadedFileFailedItsCheck => '下载的文件未通过校验，已删除。当前模型仍可使用。';

  @override
  String get theModelCouldNotBeSaved => '无法保存模型。请检查可用空间。当前模型仍可使用。';

  @override
  String get theDownloadServerRefusedTheFile => '下载服务器拒绝了该文件。当前模型仍可使用。';

  @override
  String get savedTheNextRecordingUsesThis => '已保存。下次录音将使用此模型。';

  @override
  String get savedTheNextRecordingUsesThis2 => '已保存。下次录音将使用此设置。';

  @override
  String get savedTheNextRecordingUsesThe => '已保存。下次录音将使用自动选择。';

  @override
  String get savedMeetingsRecordedFromNowOn => '已保存。今后录制的会议将使用此设置。';

  @override
  String get savedTheNextRecordingUsesThese => '已保存。下次录音将使用这些名称和术语。';

  @override
  String get eachTermCanHaveAtMost => '每个术语最多 60 个字符，总计最多 500 个字符。';

  @override
  String get chooseAWindowBetween1And => '请选择 1 到 60 天之间的期限。';

  @override
  String get theDownloadCouldNotStartPlease => '无法开始下载。请重试。';

  @override
  String get savedTheNextRecordingIsTranscribed => '已保存。下次录音将在 NPU 上转写。';

  @override
  String get savedTheNextRecordingIsTranscribed2 => '已保存。下次录音将在处理器上转写。';

  @override
  String get theNpuFilesAreNotAll => 'NPU 文件尚未全部下载。';

  @override
  String get theNpuFilesWereRemoved => 'NPU 文件已移除。';

  @override
  String get theNpuFilesCouldNotBe => '无法移除 NPU 文件。请重试。';

  @override
  String get downloadedRecordingsNowTranscribeOnThe => '已下载。录音现在在 NPU 上转写。';

  @override
  String get theDownloadStoppedNoConnectionTo2 => '下载已停止：无法连接服务器。';

  @override
  String get aDownloadedFileFailedItsCheck => '有一个下载的文件未通过校验，已删除。';

  @override
  String get theFilesCouldNotBeSaved => '无法保存文件。请检查可用空间（约需 800 MB）。';

  @override
  String get theDownloadServerRefusedTheFiles => '下载服务器拒绝了这些文件。';

  @override
  String get theNextRecordingTriesTheGraphics => '下次录音将再次尝试使用图形芯片。';

  @override
  String get thatModelWonTBeProposed => '将不再推荐该模型。';

  @override
  String get modelRemoved => '模型已移除。';

  @override
  String get anotherModelIsDownloadingWaitFor => '另一个模型正在下载。请等待其完成。';

  @override
  String get thereIsnTEnoughFreeSpace => '可用空间不足，无法存放此模型。';

  @override
  String get thatDidnTWorkPleaseTry => '操作未成功。请重试。';

  @override
  String get unableToLoadVoiceGallery => '无法加载声音库';

  @override
  String get unableToPlayThisVoice => '无法播放此声音';

  @override
  String get unableToDeleteThisVoice => '无法删除此声音';

  @override
  String get voiceDeleted => '声音已删除';

  @override
  String get unableToDeleteThisPerson => '无法删除此人';

  @override
  String deleted(Object displayName) {
    return '已删除 $displayName';
  }

  @override
  String get enterANameBeforeAssigning => '分配前请输入名称';

  @override
  String assignedTo(Object identityName) {
    return '已分配给 $identityName';
  }

  @override
  String assignedToThereWasNotEnough(Object identityName) {
    return '已分配给 $identityName。清晰语音不足，今后的录音中无法识别此声音。';
  }

  @override
  String get unableToAssignTheSpeakerCluster => '无法分配该发言人分组';

  @override
  String get chooseAtLeastOneVoiceTo => '请至少选择一个要命名的声音';

  @override
  String thereWasNotEnoughClearSpeech(Object assigned) {
    return '$assigned。清晰语音不足，今后的录音中无法识别此声音。';
  }

  @override
  String get theSettingCouldNotBeSaved => '无法保存该设置。';

  @override
  String get theAppIsStillStartingTry => '应用仍在启动。请重试。';

  @override
  String get somethingWentWrongTryAgain => '出了点问题。请重试。';

  @override
  String isUsingTheMicrophone(Object app) {
    return '$app 正在使用麦克风';
  }

  @override
  String get recordThisCall => '要录制此通话吗？';

  @override
  String theAiServerRejectedTheRequest(Object detail) {
    return 'AI 服务器拒绝了请求。\n服务器返回：$detail';
  }

  @override
  String claudeCodeOrCodexCouldNot(Object detail) {
    return 'Claude Code 或 Codex 无法回答。\n它返回：$detail';
  }

  @override
  String get typeAQuestion => '请输入问题。';

  @override
  String get theQuestionIsTooLong => '问题太长。';

  @override
  String get addAnAiConnectionInSettings => '请在“设置”中添加 AI 连接，或选择一个默认连接。';

  @override
  String get thatAiConnectionNoLongerExists => '该 AI 连接已不存在。';

  @override
  String get thereIsNothingToTalkAbout => '暂无可讨论的内容：没有转写、摘要、笔记或文件。';

  @override
  String get theAiConnectionSContextIs => '该 AI 连接的上下文太小。请选择上下文更大的连接。';

  @override
  String get thatMeetingNoLongerExists => '该会议已不存在。';

  @override
  String get thatProjectNoLongerExists => '该项目已不存在。';

  @override
  String get theAiServerCouldNotBe => '无法连接 AI 服务器。请检查连接。';

  @override
  String get theAiServerRefusedTheApi => 'AI 服务器拒绝了 API 密钥。';

  @override
  String get theAiServerIsBusyTry => 'AI 服务器繁忙。请稍后重试。';

  @override
  String get theModelDeclinedToAnswer => '模型拒绝回答。';

  @override
  String get claudeCodeOrCodexIsNot => '此电脑未安装 Claude Code 或 Codex，或应用找不到它。';

  @override
  String get theNotesCouldNotBeRead => '无法读取笔记。';

  @override
  String get noPairedComputerAnsweredOnA =>
      '没有已配对的电脑回应。请在电脑上打开“设置 → AI”，在“为我的其他设备生成摘要”中选择一个连接，并保持电脑开机和连接。';

  @override
  String get theComputerTookTooLongTo => '电脑回复超时。';

  @override
  String get theComputerDisconnectedBeforeAnsweringTry => '电脑在回复前断开了连接。请重试。';

  @override
  String get theComputerAnswersAtMost30 => '电脑每小时最多回答 30 个问题。请稍后重试。';

  @override
  String get theComputerIsAnsweringOtherQuestions => '电脑正在回答其他问题。请稍后重试。';

  @override
  String get pairedDevicesAreOffOnThis => '此手机已关闭已配对设备功能。';

  @override
  String get theAiServerCouldNotAnswer => 'AI 服务器无法回答。请重试。';

  @override
  String get exportMeeting => '导出会议';

  @override
  String get openaiCompatibleServer => 'OpenAI 兼容服务器';

  @override
  String get localServerOllama => '本地服务器（Ollama）';

  @override
  String get claudeCodeClaudePlan => 'Claude Code（Claude 订阅）';

  @override
  String get codexChatgptPlan => 'Codex（ChatGPT 订阅）';

  @override
  String get checkTheAddressAndModelPlain =>
      '请检查地址和模型。普通 http:// 仅适用于此设备或本地网络。';

  @override
  String get thatConnectionNoLongerExists => '该连接已不存在。';

  @override
  String get addAnAiConnectionInSettings2 => '请先在“设置”中添加 AI 连接。';

  @override
  String get thisMeetingHasNoTranscriptOr => '此会议还没有转写或笔记。';

  @override
  String get thisMeetingIsTooLongFor => '此会议对该连接来说太长。请选择上下文更大的连接。';

  @override
  String get theAiServerCouldNotBe2 => '无法连接 AI 服务器。';

  @override
  String get theAiServerRefusedAccessUse => 'AI 服务器拒绝访问。请检查连接的 API 密钥。';

  @override
  String get theAiServerIsBusyOr => 'AI 服务器繁忙或已超出限额。请稍后重试。';

  @override
  String get theAiServerRejectedTheRequest2 => 'AI 服务器拒绝了请求。请检查模型名称。';

  @override
  String get theAiServerFailedTryAgain => 'AI 服务器出错。请重试。';

  @override
  String get theAiServerSAnswerCould => '无法读取 AI 服务器的回复。';

  @override
  String get claudeCodeOrCodexCouldNot2 => 'Claude Code 或 Codex 无法回答。请检查是否已登录。';

  @override
  String get theSettingsCouldNotBeSaved => '无法保存设置。';

  @override
  String get theMeetingCouldNotBeRead => '无法读取或保存会议。';

  @override
  String get error => '错误';

  @override
  String get warning => '警告';

  @override
  String get infoDefault => '信息（默认）';

  @override
  String get debug => '调试';

  @override
  String get trace => '跟踪';

  @override
  String get thisDevice2 => '此设备';

  @override
  String get anotherDevice => '另一台设备';

  @override
  String notConnected(Object label) {
    return '$label（未连接）';
  }

  @override
  String itIsKeptEncryptedUntil(Object until) {
    return '加密保存至 $until。';
  }

  @override
  String get recordedOnThisDeviceOnly => '仅在此设备上录制。';

  @override
  String recordedOnThisDeviceOnlyA(Object until) {
    return '仅在此设备上录制。在另一台设备上录制的同一会议在 $until 前仍可合并。';
  }

  @override
  String combinesTheRecordingsNothingIsLeft(Object combiner) {
    return '由 $combiner 合并录音；此处已无需发送内容。';
  }

  @override
  String waitingForAudioFrom(Object names, Object kept) {
    return '正在等待来自 $names 的音频。$kept';
  }

  @override
  String combinesTheRecordingsWaitingForAudio(Object combiner, Object names) {
    return '由 $combiner 合并录音；正在等待来自 $names 的音频。';
  }

  @override
  String get audioFromEveryDeviceIsHere => '所有设备的音频均已到达；此设备空闲时将合并转写。';

  @override
  String thisDeviceSAudioStillHas(Object combiner, Object kept) {
    return '此设备的音频仍需发送到 $combiner。$kept';
  }

  @override
  String get transcriptCombinedFromEveryDevice => '已合并所有设备的转写。';

  @override
  String notCombinedAudioFromDidNot(Object plainNames) {
    return '未合并：来自 $plainNames 的音频未在三周内到达。';
  }

  @override
  String meeting(Object createdAt) {
    return '会议 $createdAt';
  }

  @override
  String get writeSomethingFirst => '请先写点内容。';

  @override
  String get theTextIsTooLong => '文本太长。';

  @override
  String get thatPartOfTheTranscriptChanged => '该段转写已被更改。请重新打开会议。';

  @override
  String get theMeetingCouldNotBeSaved => '无法保存会议。';

  @override
  String get chooseAFullFolderPathInside => '请选择完整的文件夹路径，须位于主文件夹内，但不能是主文件夹本身。';

  @override
  String get theFolderCouldNotBeWritten => '无法写入该文件夹。请检查它是否存在以及你是否有写入权限。';

  @override
  String get theNotesCouldNotBeRead2 => '无法读取或保存笔记。';

  @override
  String get theInviteAnswerIsIncomplete => '邀请的响应不完整。';

  @override
  String get theRenameAnswerIsIncomplete => '重命名的响应不完整。';

  @override
  String get thisDeviceHasNoCamera => '此设备没有摄像头。';

  @override
  String get theCameraIsNotOpen => '摄像头未打开。';

  @override
  String get title => '标题';

  @override
  String get summary => '摘要';

  @override
  String get notes => '笔记';

  @override
  String get transcript => '转写';

  @override
  String get projectName => '项目名称';

  @override
  String file(Object fileName) {
    return '文件 $fileName';
  }

  @override
  String get typeWhatToLookFor => '请输入要查找的内容。';

  @override
  String get typeAtLeastTwoLetters => '请至少输入两个字符。';

  @override
  String get theSearchIsTooLong => '搜索内容太长。';

  @override
  String get exportLogs => '导出日志';

  @override
  String about(Object appName) {
    return '关于 $appName';
  }

  @override
  String get versionLicenseAndTheSoftwareIt => '版本、许可证及所用软件。';

  @override
  String remove(Object label) {
    return '要移除 $label 吗？';
  }

  @override
  String get itsApiKeyIsDeletedFrom => '其 API 密钥将从此设备删除。已生成的摘要会保留。';

  @override
  String get cancel => '取消';

  @override
  String get remove2 => '移除';

  @override
  String get aiConnections => 'AI 连接';

  @override
  String get addConnection => '添加连接';

  @override
  String get summariesSendTheMeetingSTranscript =>
      '生成摘要时会将会议的转写、笔记和照片发送到连接的服务器。本地网络中的服务器会将它们保留在本地。';

  @override
  String get noConnectionsYet => '暂无连接';

  @override
  String get addAServerOnYourNetwork =>
      '添加你网络中的服务器（Ollama、LM Studio、llama.cpp），或此电脑上的 Claude Code 或 Codex。';

  @override
  String get summarizeMeetingsAutomatically => '自动生成会议摘要';

  @override
  String get withTheDefaultConnectionWhenA => '使用默认连接，在会议转写定稿后进行。';

  @override
  String get summariesForMyOtherDevices => '为我的其他设备生成摘要';

  @override
  String get aPairedPhoneCanAskThis =>
      '已配对的手机可以请求此电脑使用此连接、根据此电脑上的笔记副本生成摘要或回答聊天。这会计入该订阅的用量；每小时最多 10 份摘要和 30 次聊天回答。';

  @override
  String get off => '关闭';

  @override
  String get summaryPrompt => '摘要提示词';

  @override
  String placeholdersTheTranscriptIsAlwaysSent(Object placeholders) {
    return '占位符：$placeholders。转写始终在提示词之后发送。';
  }

  @override
  String get savePrompt => '保存提示词';

  @override
  String get resetToBuiltIn => '恢复内置';

  @override
  String get defaultLabel => '默认';

  @override
  String get installed => '已安装';

  @override
  String get notFoundOnThisComputer => '此电脑上未找到';

  @override
  String get unencryptedHttpLocalNetworkOnly => '未加密的 http，仅限本地网络';

  @override
  String get runsWithYourPlanSSign => '使用此电脑上你的订阅登录运行';

  @override
  String actionsFor(Object label) {
    return '$label 的操作';
  }

  @override
  String get makeDefault => '设为默认';

  @override
  String get test => '测试';

  @override
  String get edit => '编辑';

  @override
  String get enterANameAnAddressAnd => '请输入名称、地址和模型。';

  @override
  String get theContextIsANumberOf => '上下文是一个 token 数，例如 8192。';

  @override
  String edit2(Object label) {
    return '编辑 $label';
  }

  @override
  String get name => '名称';

  @override
  String get protocol => '协议';

  @override
  String get runsTheToolInstalledAndSigned =>
      '运行此电脑上已安装并登录的工具，由你的 Claude 或 ChatGPT 订阅生成摘要，而非使用 API 额度。会议内容将以该账户发送给 Anthropic 或 OpenAI。';

  @override
  String get address => '地址';

  @override
  String get httpsAlwaysHttpOnlyOnThis => '始终可用 https://；http:// 仅限此设备或本地网络。';

  @override
  String get model => '模型';

  @override
  String get leaveEmptyForTheToolS => '留空则使用该工具的默认模型。';

  @override
  String get contextTokens => '上下文（token）';

  @override
  String get saveConnection => '保存连接';

  @override
  String get koalinoIsLocked => 'Koalino 已锁定';

  @override
  String get unlockWithYourFingerprintFaceOr => '使用指纹、面容或屏幕锁定解锁。';

  @override
  String get unlock => '解锁';

  @override
  String copied(Object what) {
    return '已复制 $what';
  }

  @override
  String get aiAssistantAccess => 'AI 助手访问';

  @override
  String get letYourOwnAiAssistantSuch =>
      '允许你自己的 AI 助手（如 Claude Code、Codex 或 Claude Desktop）读取你的会议和项目：标题、摘要、笔记、转写和项目文件。它只能读取，不能更改或删除任何内容。';

  @override
  String get whatTheAssistantReadsMayBe =>
      '助手读取的内容可能会以你在该服务的账户发送到其在线模型。请仅为你信任、可交付会议内容的助手开启。';

  @override
  String get theAssistantServerIsNotInstalled => '此应用副本未安装助手服务器。';

  @override
  String get letMyAiAssistantReadMy => '允许我的 AI 助手读取我的笔记';

  @override
  String get connectYourAssistant => '连接你的助手';

  @override
  String get yourNotesAreUnlockedWithA =>
      '你的笔记由密码短语解锁，该密码短语绝不会写入助手的设置。只有在助手启动服务器的环境中设置了 NOTE_TAKER_ENCRYPTION_PASSPHRASE 时，服务器才会读取笔记。';

  @override
  String get claudeCodeRunInATerminal => 'Claude Code：在终端中运行';

  @override
  String get command => '命令';

  @override
  String get claudeDesktopAndOtherMcpClients =>
      'Claude Desktop 及其他 MCP 客户端：添加到其设置中';

  @override
  String get settings => '设置';

  @override
  String copy(Object title) {
    return '复制：$title';
  }

  @override
  String isUsingTheMicrophoneRecordThis(Object app) {
    return '$app 正在使用麦克风。要录制此通话吗？';
  }

  @override
  String get notNow => '暂不';

  @override
  String get record => '录制';

  @override
  String get offerToRecordCalls => '提示录制通话';

  @override
  String get whenTeamsZoomMeetInA =>
      '当 Teams、Zoom、浏览器中的 Meet 或其他通话应用开始使用麦克风时。在你点按“录制”前不会录制任何内容。';

  @override
  String get stopWhenTheCallEnds => '通话结束时停止';

  @override
  String get onlyForRecordingsStartedFromThe =>
      '仅适用于从提示开始的录音，在通话应用释放麦克风 20 秒后停止。';

  @override
  String get readSpeakerNamesFromTheCall => '从通话窗口读取发言人姓名';

  @override
  String get whileRecordingKoalinoReadsWhoTeams =>
      '录音时，Koalino 会读取 Teams、Zoom 或 Meet 显示的发言人，为未知声音推荐姓名。所有数据都不会离开此电脑，在你点按“分配”前不会为任何人命名。';

  @override
  String get deleteThisChat => '要删除此聊天吗？';

  @override
  String get itsQuestionsAndAnswersAreDeleted => '其问题和回答将从此设备删除。';

  @override
  String get delete => '删除';

  @override
  String get thatFileIsNoLongerIn => '该文件已不在项目中。';

  @override
  String get whoAnswers => '回答者';

  @override
  String get myPairedComputer => '我已配对的电脑';

  @override
  String get thisPhoneSAiConnection => '此手机的 AI 连接';

  @override
  String get deleteThisChat2 => '删除此聊天';

  @override
  String get askAboutYourNotesAnswersCome =>
      '就你的笔记提问。回答来自你已配对的电脑，它会用在那里为其他设备选择的连接读取自己的笔记副本。';

  @override
  String get askAboutYourNotesTheAnswer =>
      '就你的笔记提问。回答来自你的默认 AI 连接，会向其发送放得下的笔记。';

  @override
  String get yourQuestion => '你的问题';

  @override
  String get ask => '提问';

  @override
  String get open => '打开';

  @override
  String given(Object included) {
    return '已提供：$included';
  }

  @override
  String leftOutForLength(Object omitted) {
    return '因长度未包含：$omitted';
  }

  @override
  String answeredBy(Object connection) {
    return '由 $connection 回答';
  }

  @override
  String get unableToOpenTheKeychain => '无法打开钥匙串';

  @override
  String get koalinoKeepsTheKeyThatEncrypts =>
      'Koalino 将加密你的声纹档案和设备身份的密钥保存在系统钥匙串中。请解锁钥匙串，或允许 Koalino 使用它，然后重试。如果此设备没有钥匙串服务，请安装或启动一个，然后重启 Koalino。';

  @override
  String get unableToOpenYourNotes => '无法打开你的笔记';

  @override
  String get koalinoCouldNotOpenItsDatabase =>
      'Koalino 无法打开其数据库。请检查是否有可用空间，然后重试。';

  @override
  String get unlockingYourNotes => '正在解锁你的笔记…';

  @override
  String get quit => '退出';

  @override
  String get retry => '重试';

  @override
  String delete2(Object title) {
    return '要删除 $title 吗？';
  }

  @override
  String get itsTranscriptAttachmentsAndUnknownVoices =>
      '其转写、附件和未知声音将被永久删除，包括此处和你已配对的设备。已保存的人员会保留。';

  @override
  String get unableToDeleteThisMeetingIf => '无法删除此会议。如果正在录制，请先停止录音。';

  @override
  String get devices => '设备';

  @override
  String get deviceSyncIsOff => '设备同步已关闭';

  @override
  String get meetingsStayOnThisDeviceUntil => '开启同步前，会议只保留在此设备上。';

  @override
  String thisDevice3(Object capabilities) {
    return '此设备\n$capabilities';
  }

  @override
  String get renameThisDevice => '重命名此设备';

  @override
  String get discoverableYourOtherDevicesCanAsk => '可被发现：你的其他设备可在几分钟内请求配对。';

  @override
  String get makeDiscoverable => '设为可被发现';

  @override
  String get pairingRequests => '配对请求';

  @override
  String get acceptOnlyIfTheOtherDevice => '仅当另一台设备显示相同代码时才接受。';

  @override
  String get reject => '拒绝';

  @override
  String get accept => '接受';

  @override
  String get pairingInProgress => '正在配对';

  @override
  String get waitingForTheOtherDevice => '正在等待另一台设备…';

  @override
  String get checkThatTheOtherDeviceShows => '请检查另一台设备是否显示相同代码。';

  @override
  String get cancelPairing => '取消配对';

  @override
  String get codesMatch => '代码一致';

  @override
  String get pairedDevices => '已配对设备';

  @override
  String get noPairedDevicesYet => '暂无已配对设备。';

  @override
  String paired(Object pairedAt, Object hasRelay) {
    return '配对于 $pairedAt\n$hasRelay';
  }

  @override
  String remove3(Object deviceName) {
    return '移除 $deviceName';
  }

  @override
  String get nearbyDevices => '附近的设备';

  @override
  String get pair => '配对';

  @override
  String get anotherNetwork => '其他网络';

  @override
  String get showAnInviteHereAndUse => '在此显示邀请，并在另一台设备上使用。配对前你仍需比对代码。';

  @override
  String get showInvite => '显示邀请';

  @override
  String get useInvite => '使用邀请';

  @override
  String stopsSyncingWithThisDevicePair(Object deviceName) {
    return '$deviceName 将停止与此设备同步。重新配对即可恢复。';
  }

  @override
  String pairingCode(Object split) {
    return '配对代码 $split';
  }

  @override
  String get noOtherDevicesFoundOnThis =>
      '此网络中未找到其他设备。“附近”只能找到同一 Wi-Fi 中的设备。对于其他网络中的设备，请使用邀请。';

  @override
  String get noOtherDevicesFoundOnThis2 =>
      '此网络中未找到其他设备。“附近”只能找到同一 Wi-Fi 中的设备。对于其他网络中的设备，请使用邀请或在“设置”中设置中继。';

  @override
  String get notConnectedItReconnectsWhenIt => '未连接。它联网并能访问你的中继时会重新连接。';

  @override
  String get notConnectedPutBothDevicesOn =>
      '未连接。请将两台设备连到同一 Wi-Fi，或在“设置”中设置中继。';

  @override
  String get whisperOnDevice => 'Whisper（设备端）';

  @override
  String get openaiCloud => 'OpenAI（云端）';

  @override
  String gbGpu(Object round) {
    return '$round GB GPU';
  }

  @override
  String mbGpu(Object gpuMegabytes) {
    return '$gpuMegabytes MB GPU';
  }

  @override
  String get noTranscriptionModels => '没有转写模型';

  @override
  String get useYourOwnAiApp => '使用你自己的 AI 应用';

  @override
  String get theMeetingItsNotesAndIts =>
      '会议及其笔记和文件将以你自己的账户发送到你选择的应用。如需摘要，请在那里复制回答后返回，或将其分享给 Koalino：Koalino 会保存它。';

  @override
  String summarizeIn(Object displayName) {
    return '在 $displayName 中生成摘要';
  }

  @override
  String chatIn(Object displayName) {
    return '在 $displayName 中聊天';
  }

  @override
  String get summarizeInAnotherApp => '在其他应用中生成摘要…';

  @override
  String get chatInAnotherApp => '在其他应用中聊天…';

  @override
  String summaryFromSavedTo(Object appName, Object title) {
    return '来自 $appName 的摘要已保存到“$title”。';
  }

  @override
  String get undo => '撤销';

  @override
  String get theEarlierSummaryIsBack => '已恢复之前的摘要。';

  @override
  String toKeepTheSummaryCopyS(Object appName) {
    return '如需保留摘要，请复制 $appName 的回答并返回 Koalino，或将回答分享给 Koalino。';
  }

  @override
  String get meetings => '会议';

  @override
  String get projects => '项目';

  @override
  String get voices => '声音';

  @override
  String get meeting2 => '会议';

  @override
  String get chatAboutThisMeeting => '就此会议聊天';

  @override
  String chat(Object meeting) {
    return '聊天：$meeting';
  }

  @override
  String get thisMeeting => '此会议';

  @override
  String get takePhotos => '拍照';

  @override
  String get attachFiles => '附加文件';

  @override
  String get export => '导出';

  @override
  String shareAs(Object label) {
    return '以 $label 分享';
  }

  @override
  String get nameSpeakers => '为发言人命名';

  @override
  String get selectAMeetingToSeeIts => '选择一个会议以查看其转写。';

  @override
  String get renameMeeting => '重命名会议';

  @override
  String get deleteMeeting => '删除会议';

  @override
  String recordedOn(Object devices) {
    return '录制于 $devices';
  }

  @override
  String get split => '拆分';

  @override
  String get longPressOrRightClickA => '长按或右键点击某段内容即可更正。';

  @override
  String get noTranscriptIsAvailable => '暂无转写。';

  @override
  String get theSpeakerIsAGuess => '发言人为推测';

  @override
  String get peopleTalkedAtOnceHereWords => '此处多人同时说话；可能缺少字词';

  @override
  String get overlappingSpeech => '重叠语音';

  @override
  String get someWordsWereUnclear => '部分字词不清晰';

  @override
  String get addedAfterTheMeeting => '会后添加';

  @override
  String photoOpenFullScreen(Object fileName) {
    return '照片 $fileName，全屏打开';
  }

  @override
  String get save => '保存';

  @override
  String get preview => '预览';

  @override
  String get editNotes => '编辑笔记';

  @override
  String get writeNotes => '写笔记';

  @override
  String get loading => '正在加载…';

  @override
  String get noNotesYet => '暂无笔记。';

  @override
  String textAt(Object startTimeMs) {
    return '$startTimeMs 处的文本';
  }

  @override
  String get removedThisTranscriptWonTBe => '已移除。会后不会重新转写此转写。';

  @override
  String get savedThisTranscriptWonTBe => '已保存。会后不会重新转写此转写。';

  @override
  String get editText => '编辑文本';

  @override
  String get editTheTranscript => '编辑转写';

  @override
  String get leaveItEmptyToRemoveThis => '留空即可移除此部分。';

  @override
  String get searchMeetings => '搜索会议';

  @override
  String get chatWithAllYourNotes => '与你的所有笔记聊天';

  @override
  String get chatWithAllNotes => '与所有笔记聊天';

  @override
  String get refresh => '刷新';

  @override
  String get noMeetingsYet => '暂无会议';

  @override
  String get recordingsYouStartAppearHere => '你开始的录音会显示在这里。';

  @override
  String get rename => '重命名';

  @override
  String get addToProject => '添加到项目…';

  @override
  String get summary2 => '摘要…';

  @override
  String get addToProject2 => '添加到项目';

  @override
  String writingTheSummaryOf(Object title) {
    return '正在生成 $title 的摘要';
  }

  @override
  String summaryOf(Object title) {
    return '$title 的摘要';
  }

  @override
  String summarizeWith(Object label) {
    return '使用 $label 生成摘要';
  }

  @override
  String defaultLabel2(Object model) {
    return '$model · 默认';
  }

  @override
  String get summarizeOnMyComputer => '在我的电脑上生成摘要';

  @override
  String get withClaudeCodeCodexOrThe => '使用 Claude Code、Codex 或在那里设置的连接';

  @override
  String get useYourOwnAiApp2 => '使用你自己的 AI 应用…';

  @override
  String get claudeChatgptOrAnotherAppWith => 'Claude、ChatGPT 或其他应用，使用你的账户';

  @override
  String get openSummary => '打开摘要';

  @override
  String get editPromptForThisMeeting => '编辑此会议的提示词…';

  @override
  String get useTheSettingsPrompt => '使用“设置”中的提示词';

  @override
  String get promptForThisMeeting => '此会议的提示词';

  @override
  String placeholders(Object placeholders) {
    return '占位符：$placeholders';
  }

  @override
  String get addAnAiConnectionInSettings3 => '请在“设置”中添加 AI 连接以生成摘要。';

  @override
  String get summarizeWith2 => '生成摘要使用';

  @override
  String get summarize => '生成摘要';

  @override
  String get summarizeAgain => '重新生成摘要';

  @override
  String get editPromptForThisMeeting2 => '编辑此会议的提示词';

  @override
  String get writingTheSummaryALocalModel => '正在生成摘要… 本地模型可能需要几分钟。';

  @override
  String get noSummaryYet => '暂无摘要。';

  @override
  String get writeASummary => '写摘要';

  @override
  String writtenByYouOn(Object createdAt) {
    return '由你写于 $createdAt';
  }

  @override
  String writtenByOn(
      Object model, Object connectionLabel, Object createdAt, Object edited) {
    return '由 $model（$connectionLabel）写于 $createdAt$edited';
  }

  @override
  String get editSummary => '编辑摘要';

  @override
  String get tryAgainOnMyComputer => '在我的电脑上重试';

  @override
  String get cancelRequest => '取消请求';

  @override
  String get notesFolder => '笔记文件夹';

  @override
  String get keepYourProjectsAndMeetingsAs =>
      '将项目和会议以普通文件保存在一个文件夹中，可用任何编辑器阅读和编辑。对 notes.md、summary.md 和项目文件的修改会在半分钟内同步回应用；放入项目文件夹或会议附件文件夹的文件会被添加。transcript.md 由应用重写：请在应用中更正转写。';

  @override
  String get theFolderIsADecryptedCopy =>
      '该文件夹是笔记的解密副本。任何能读取它的人都能读取你的笔记，备份或同步工具也会复制它。请将其放在加密磁盘上你自己的用户文件夹内。';

  @override
  String get folder => '文件夹';

  @override
  String get chooseAFolder => '选择文件夹';

  @override
  String get keepTheNotesFolder => '维护笔记文件夹';

  @override
  String get syncNow => '立即同步';

  @override
  String lastSyncAtFailed(Object at, Object code) {
    return '$at 的上次同步失败：$code';
  }

  @override
  String lastSyncAtFilesWrittenEdits(
      Object at, Object written, Object imported) {
    return '上次同步于 $at：写入 $written 个文件，导入 $imported 处修改和文件。';
  }

  @override
  String get inviteADevice => '邀请设备';

  @override
  String get pairingInviteQrCode => '配对邀请二维码';

  @override
  String onTheOtherDeviceOpenDevices(Object hour, Object minute) {
    return '在另一台设备上打开“设备”，选择“使用邀请”，然后粘贴此链接。你可以用相机应用扫描二维码来复制它。有效期至 $hour:$minute。';
  }

  @override
  String get close => '关闭';

  @override
  String get inviteLinkCopied => '邀请链接已复制';

  @override
  String get copyLink => '复制链接';

  @override
  String get deviceFoundChoosePairUnderNearby => '已找到设备。请在“附近的设备”中选择“配对”。';

  @override
  String get useAnInvite => '使用邀请';

  @override
  String get inviteLink => '邀请链接';

  @override
  String get findDevice => '查找设备';

  @override
  String get pairWithAnotherDevice => '要与另一台设备配对吗？';

  @override
  String get youOpenedAPairingInviteKoalino =>
      '你打开了一个配对邀请。Koalino 会找到显示该邀请的设备，在共享任何内容前，两台设备都会显示一个代码供你比对。';

  @override
  String get pairingStartedCheckThatBothDevices => '已开始配对。请检查两台设备是否显示相同代码。';

  @override
  String get done => '完成';

  @override
  String get photosAreAddedToTheMeeting => '照片拍摄后会立即添加到会议。';

  @override
  String get takePhoto => '拍照';

  @override
  String get theCameraIsUnavailable => '摄像头不可用。';

  @override
  String search(Object name) {
    return '搜索 $name';
  }

  @override
  String get noteSaved => '笔记已保存';

  @override
  String get theNoteCouldNotBeSaved => '无法保存笔记。';

  @override
  String couldNotBeAdded2(Object failed) {
    return '$failed 个无法添加。';
  }

  @override
  String get renameProject => '重命名项目';

  @override
  String get theProjectCouldNotBeRenamed => '无法重命名项目。';

  @override
  String delete3(Object project) {
    return '要删除“$project”吗？';
  }

  @override
  String get itsFilesNotesAndEMails => '其文件、笔记和邮件将在所有已配对设备上删除。其会议会保留，但不属于任何项目。';

  @override
  String get theProjectCouldNotBeDeleted => '无法删除项目。';

  @override
  String get itIsDeletedOnEveryPaired => '将在所有已配对设备上删除。';

  @override
  String get theFileCouldNotBeDeleted => '无法删除文件。';

  @override
  String get theMeetingCouldNotBeRemoved => '无法将会议从项目中移除。';

  @override
  String get project => '项目';

  @override
  String get searchThisProject => '搜索此项目';

  @override
  String get chatAboutThisProject => '就此项目聊天';

  @override
  String get addFiles => '添加文件';

  @override
  String get writeANote => '写笔记';

  @override
  String get addMeetings => '添加会议';

  @override
  String get deleteProject => '删除项目';

  @override
  String get addThemHereOrFromA => '在此处添加，或从会议页面添加。';

  @override
  String get removeFromProject => '从项目中移除';

  @override
  String get filesNotesAndEMails => '文件、笔记和邮件';

  @override
  String get nothingAddedYet => '尚未添加任何内容';

  @override
  String get addPicturesMarkdownPdfsEMails => '添加图片、Markdown、PDF、邮件（.eml）或文档。';

  @override
  String get onItsWayFromAPaired => '正在从已配对设备传来';

  @override
  String get newNote => '新建笔记';

  @override
  String get noteMarkdown => '笔记（Markdown）';

  @override
  String get everyMeetingIsAlreadyInA => '所有会议都已在项目中。';

  @override
  String get add => '添加';

  @override
  String get moveToProject => '移到项目';

  @override
  String get noProject => '无项目';

  @override
  String get newProject => '新建项目…';

  @override
  String get newProject2 => '新建项目';

  @override
  String get create => '创建';

  @override
  String get theProjectCouldNotBeCreated => '无法创建项目。';

  @override
  String get theMeetingCouldNotBeMoved => '无法移动会议。';

  @override
  String get removedFromItsProject => '已从其项目中移除';

  @override
  String movedTo(Object project) {
    return '已移到 $project';
  }

  @override
  String get searchProjects => '搜索项目';

  @override
  String get noProjectsYetAProjectGathers => '暂无项目。项目会将会议与相关的文件、笔记和邮件归在一起。';

  @override
  String get theRecordingIsStillStartingTry => '录音仍在启动；请重试。';

  @override
  String recording(Object title) {
    return '正在录制“$title”';
  }

  @override
  String get recording2 => '正在录制';

  @override
  String paused(Object title) {
    return '“$title”已暂停';
  }

  @override
  String get paused2 => '已暂停';

  @override
  String get finishingTranscription => '正在完成转写…';

  @override
  String get meetingTitle => '会议标题';

  @override
  String get enterMeetingTitleOptional => '输入会议标题（可选）';

  @override
  String get leaveEmptyToNameTheMeeting => '留空则以开始时间命名会议';

  @override
  String get startingRecording => '正在开始录音';

  @override
  String get startRecording => '开始录音';

  @override
  String get pause => '暂停';

  @override
  String get resume => '继续';

  @override
  String get stop => '停止';

  @override
  String get language => '语言';

  @override
  String get aChangeAppliesToWhatIs => '更改将应用于之后说的内容';

  @override
  String get liveTranscript => '实时转写';

  @override
  String get theTranscriptAppearsHereAFew => '每次说话停顿几秒后，转写会显示在这里。';

  @override
  String get nearbyDevicesSeeThisNameWhile =>
      '此设备可被发现时，附近的设备会看到此名称。已配对的设备会保留它们已知的名称。';

  @override
  String get meetingRenamed => '会议已重命名';

  @override
  String get thisTitleIsnTAllowed => '不允许使用此标题。';

  @override
  String get couldnTSaveTheNewTitle => '无法保存新标题。请重试。';

  @override
  String get theNoteTakingEngineIsnT => '笔记引擎暂时不可用。';

  @override
  String get thisMeetingNoLongerExists2 => '此会议已不存在。';

  @override
  String get couldnTRenameTheMeeting => '无法重命名会议。';

  @override
  String get thisFileIsStillOnIts => '此文件仍在从已配对设备传来。';

  @override
  String get thisPictureCannotBeShown => '无法显示此图片。';

  @override
  String get thisKindOfDocumentCanT => '应用暂时无法显示此类文档。';

  @override
  String get thisFileCannotBeOpened => '无法打开此文件。';

  @override
  String get thisFileCannotBeRead => '无法读取此文件。';

  @override
  String get thisEMailCannotBeRead => '无法读取此邮件。';

  @override
  String get noSubject => '（无主题）';

  @override
  String get from => '发件人';

  @override
  String get to => '收件人';

  @override
  String get cc => '抄送';

  @override
  String get sent => '发送时间';

  @override
  String attachments(Object attachmentNames) {
    return '附件：$attachmentNames';
  }

  @override
  String get theRestOfThisEMail => '此邮件其余部分过长，无法显示。';

  @override
  String get searching => '正在搜索…';

  @override
  String get nothingFound => '未找到任何内容。';

  @override
  String get search2 => '搜索';

  @override
  String get pairYourDevicesAndSeeWhich => '配对你的设备并查看哪些已连接。';

  @override
  String get transcriptionModel => '转写模型';

  @override
  String get chooseTheWhisperModel => '选择 Whisper 模型。';

  @override
  String get languageModelsForMeetingSummariesAnd => '用于会议摘要和聊天的语言模型，以及它们的提示词。';

  @override
  String get keepProjectsAndMeetingsAsFiles => '将项目和会议保存为可用任何编辑器编辑的文件。';

  @override
  String get letYourOwnAssistantSuchAs => '允许你自己的助手（如 Claude Code）读取你的笔记。';

  @override
  String get appLock => '应用锁';

  @override
  String get defaultLanguage => '默认语言';

  @override
  String get usedForNewRecordingsAutoDetects => '用于新录音。“自动”会在说话时检测语言。';

  @override
  String get transcribeOn => '转写设备';

  @override
  String get anotherDeviceReceivesTheRecordingS =>
      '另一台设备接收加密的录音音频，并在转写后删除。若其不可用，则由此设备转写。';

  @override
  String get telemetry => '遥测';

  @override
  String get howLongTranscriptionTakesOnEach => '每台设备上转写所需的时间及其估计能耗。';

  @override
  String get logLevel => '日志级别';

  @override
  String get howMuchDetailTheAppWrites =>
      '应用写入日志的详细程度。“调试”和“跟踪”有助于诊断问题，但会写入更多内容。';

  @override
  String get custom => '自定义';

  @override
  String get shareLogs => '分享日志';

  @override
  String get theLast7DaysOfDiagnostic => '最近 7 天的诊断日志，例如用于附加到错误报告。';

  @override
  String get unableToReadTheAppLock => '无法读取应用锁设置。';

  @override
  String get needsAndroid10OrLater => '需要 Android 10 或更高版本。';

  @override
  String get setAScreenLockInAndroid => '请在 Android 设置中设置屏幕锁定以使用应用锁。';

  @override
  String askForYourFingerprintFaceOr(Object lockAfterBackground) {
    return '离开 $lockAfterBackground 秒后返回 Koalino 时，要求验证指纹、面容或屏幕锁定。锁定期间录音和同步会继续运行。';
  }

  @override
  String get otherNetworks => '其他网络';

  @override
  String get aRelayOnYourOwnAlways =>
      '在你自己的常开设备上运行中继，可让你的设备在不同 Wi-Fi 下互相访问。同一网络内的同步无需中继也能正常工作。';

  @override
  String get relayAddress => '中继地址';

  @override
  String get deviceAddressesOptionalOnePerLine => '设备地址（可选，每行一个）';

  @override
  String get saveAddresses => '保存地址';

  @override
  String get nothingThatWasSharedCouldBe => '无法读取分享的任何内容。';

  @override
  String couldNotBeRead(Object message, Object failedCount) {
    return '$message；$failedCount 个无法读取';
  }

  @override
  String saveAsTheSummaryOf(Object title) {
    return '保存为“$title”的摘要';
  }

  @override
  String get addToAProjectInstead => '改为添加到项目';

  @override
  String get splitThisMeeting => '要拆分此会议吗？';

  @override
  String itWasRecordedOnAndThe(Object devices, Object recordings) {
    return '此会议录制于 $devices，应用已合并这些录音。拆分后，另一份录音（$recordings）会重新单独列出，并带有自己的转写，包括此处和你已配对的设备。它们不会再次合并。此会议保留其转写。';
  }

  @override
  String get unableToSplitThisMeeting => '无法拆分此会议。';

  @override
  String get telemetryDashboard => '遥测仪表板';

  @override
  String get transcription => '转写';

  @override
  String get latency => '延迟';

  @override
  String get tasks => '任务';

  @override
  String get power => '能耗';

  @override
  String get transcriptionDurationOverTime => '转写时长趋势';

  @override
  String get latencyDistribution => '延迟分布';

  @override
  String get taskTypeBreakdown => '任务类型细分';

  @override
  String get powerConsumptionEstimate => '能耗估算';

  @override
  String get fastestLeastAccurate => '最快，准确度最低';

  @override
  String get fastManyMistakesTheAppShips => '快速，错误较多。应用自带此模型';

  @override
  String get fastManyMistakes => '快速，错误较多';

  @override
  String get goodBalanceOnMostComputers => '在大多数电脑上表现均衡';

  @override
  String get accurateSlowerNeedsAbout2Gb => '准确，较慢，约需 2 GB 内存';

  @override
  String get olderNeedsAPowerfulComputer => '较旧，需要高性能电脑';

  @override
  String get mostAccurateNeedsAPowerfulComputer => '最准确，需要高性能电脑';

  @override
  String get veryAccurateBestWithAGraphics => '非常准确。最好配合显卡使用';

  @override
  String get thisModelMayBeTooLarge => '此模型可能过大';

  @override
  String mayNeedMoreMemoryThanThis(Object model) {
    return '$model 可能需要超过此设备的内存，录音可能失败或运行缓慢。';
  }

  @override
  String get downloadAnyway => '仍然下载';

  @override
  String get transcriptionSettingsAreUnavailable => '转写设置不可用。';

  @override
  String get downloadFromQualcomm => '要从 Qualcomm 下载吗？';

  @override
  String get about360MbIsDownloadedFrom =>
      '将从 Qualcomm 下载约 360 MB，占用约 460 MB 空间：\n\n• Qualcomm 的 QNN 运行时，让应用可以使用 NPU。它采用 Qualcomm 的 AI Stack License，而非本应用的许可证。\n• Whisper Small，由 Qualcomm AI Hub 为此手机的 NPU 编译。\n\n每个文件在使用前都会校验。你的录音绝不会离开手机。下载即表示你接受 Qualcomm 的条款。';

  @override
  String get acceptAndDownload => '接受并下载';

  @override
  String get downloadingTheNpuFiles => '正在下载 NPU 文件…';

  @override
  String get transcribeOnTheNpu => '在 NPU 上转写';

  @override
  String get thisPhoneHasAnAiChip =>
      '此手机有一颗 AI 芯片（NPU），可以运行 Whisper Small 并释放处理器：30 秒语音约需 2 秒。需要从 Qualcomm 下载文件。';

  @override
  String get download => '下载';

  @override
  String get runsWhisperSmallOnThePhone =>
      '在手机的 AI 芯片上运行 Whisper Small，替代下方所选的模型；会后处理若没有单独的模型也使用它。字词时间为近似值。';

  @override
  String get removeTheNpuFiles => '移除 NPU 文件';

  @override
  String get alsoRecordTheComputerSSound => '同时录制电脑声音';

  @override
  String get transcribesWhatTheComputerPlaysSuch =>
      '除麦克风外，还转写电脑播放的声音，例如通话的另一方。当麦克风也能听到扬声器时，重复的字词只保留一次。';

  @override
  String get improveTheTranscriptAfterTheMeeting => '会后改进转写';

  @override
  String get transcribesEachMeetingAgainWithMore =>
      '每次会议结束后，用更多上下文重新转写；手机会等到充电时进行。在此之前语音加密保存，之后删除。';

  @override
  String get modelForThisPass => '此次处理使用的模型';

  @override
  String get aLargerModelIsSlowerBut => '较大的模型更慢但更准确，会后有充足时间。请在上方下载模型。';

  @override
  String get sameAsForRecording => '与录音相同';

  @override
  String get keepSpeechForCombining => '保留语音以便合并';

  @override
  String get whenAMeetingWasAlsoRecorded =>
      '当会议也在另一台设备上录制时，其语音会等待这段时间以便合并，之后删除。';

  @override
  String get whisperModel => 'Whisper 模型';

  @override
  String get biggerModelsMakeFewerMistakesAnd =>
      '模型越大错误越少，但需要更多时间和内存。模型从 Hugging Face 下载。此选择仅适用于此设备。';

  @override
  String automatic2(Object fileName, Object mode) {
    return '自动：$fileName$mode';
  }

  @override
  String usesUntilIsDownloadedTheBest(
      Object fileName, Object waiting, Object reason) {
    return '在下载完最适合此设备的 $waiting 之前使用 $fileName。$reason';
  }

  @override
  String picksTheBestModelForThis(Object fileName, Object mode) {
    return '为此设备选择最佳模型：$fileName$mode。';
  }

  @override
  String download2(Object sizeBytes) {
    return '下载 $sizeBytes';
  }

  @override
  String tryLabel(Object model) {
    return '试用 $model';
  }

  @override
  String get use => '使用';

  @override
  String suggestedForThisDevice(Object model, Object mode) {
    return '推荐此设备使用：$model$mode';
  }

  @override
  String get efficientMode => '高效模式';

  @override
  String runsTheSameModelsOnThe(Object gpuName) {
    return '在图形芯片（$gpuName）上运行相同的模型，通常更快。关闭时照常使用处理器。';
  }

  @override
  String get theGraphicsChipStoppedTheApp => '上次图形芯片导致应用停止，因此此设备改用处理器转写。';

  @override
  String get tryTheGraphicsChipAgain => '再次尝试图形芯片';

  @override
  String get onThisDevice => '在此设备上';

  @override
  String get usedByAutomatic => '由“自动”使用';

  @override
  String get englishOnly => '仅英语';

  @override
  String get mayBeTooLargeForThis => '对此设备可能过大';

  @override
  String get doesnTMatchYourLanguage => '与你的语言不符';

  @override
  String get notEnoughFreeSpace => '可用空间不足';

  @override
  String get cancelDownload => '取消下载';

  @override
  String get glossary => '术语表';

  @override
  String get namesProjectNamesAndTermsOne => '人名、项目名称和术语，每行一个。转写会按此处的写法拼写。';

  @override
  String get saveGlossary => '保存术语表';

  @override
  String get voiceGallery => '声音库';

  @override
  String get tryAgain => '重试';

  @override
  String get unknownVoices => '未知声音';

  @override
  String get noUnknownSpeakersToReview => '没有需要查看的未知发言人';

  @override
  String get savedPeople => '已保存的人员';

  @override
  String get noSavedPeopleYetNameA => '暂无已保存的人员。为声音命名即可添加人员。';

  @override
  String delete4(Object displayName) {
    return '删除 $displayName';
  }

  @override
  String get playing => '正在播放…';

  @override
  String get untitledMeeting => '未命名会议';

  @override
  String get play => '播放';

  @override
  String get deleteThisVoice => '删除此声音';

  @override
  String theCallShowedSpeakingCheckThen(Object suggestedName) {
    return '通话显示 $suggestedName 正在发言。请核对后点按“分配”。';
  }

  @override
  String get chooseASavedPerson => '选择已保存的人员';

  @override
  String get assign => '分配';

  @override
  String get deleteTheseVoices => '删除这些声音';

  @override
  String get theseSoundLikeOnePersonUntick => '这些听起来像同一个人。请取消勾选属于其他人的声音。';

  @override
  String itsVoiceSamplesGoForGood(Object voices) {
    return '其 $voices 个声音样本将被永久删除，包括此处和你已配对的设备。转写会保留。';
  }

  @override
  String get itsVoiceSampleGoesForGood => '其声音样本将被永久删除，包括此处和你已配对的设备。转写会保留。';

  @override
  String get theirNameAndVoiceSamplesGo =>
      '其姓名和声音样本将被永久删除，包括此处和你已配对的设备。转写会保留，并将其编号为一位发言人。';

  @override
  String unknownSpeaker(Object index) {
    return '未知发言人 $index';
  }

  @override
  String attachedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已附加 $count 个文件',
      one: '已附加 1 个文件',
    );
    return '$_temp0';
  }

  @override
  String addedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已添加 $count 个文件',
      one: '已添加 1 个文件',
    );
    return '$_temp0';
  }

  @override
  String emailAttachmentsLeft(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个邮件附件保留在邮件中',
      one: '1 个邮件附件保留在邮件中',
    );
    return '$_temp0';
  }

  @override
  String addedMeetings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已添加 $count 个会议',
      one: '已添加 1 个会议',
    );
    return '$_temp0';
  }

  @override
  String otherRecordings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 份录音',
      one: '1 份录音',
    );
    return '$_temp0';
  }

  @override
  String get severalDevices => '多台设备';

  @override
  String meetingCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个会议',
      one: '1 个会议',
    );
    return '$_temp0';
  }

  @override
  String fileCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个文件',
      one: '1 个文件',
    );
    return '$_temp0';
  }

  @override
  String chunkCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个片段',
      one: '1 个片段',
    );
    return '$_temp0';
  }

  @override
  String resultCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个结果',
      one: '1 个结果',
    );
    return '$_temp0';
  }

  @override
  String get moreNotShown => '，还有更多未显示：添加字词以缩小范围';

  @override
  String cpuCores(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个 CPU 核心',
      one: '1 个 CPU 核心',
    );
    return '$_temp0';
  }

  @override
  String dayCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 天',
      one: '1 天',
    );
    return '$_temp0';
  }

  @override
  String voiceSampleCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个声音样本',
      one: '1 个声音样本',
      zero: '尚无声音样本，因此无法通过声音识别',
    );
    return '$_temp0';
  }

  @override
  String picturesIncluded(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '包含了 $count 张图片',
      one: '包含了 1 张图片',
    );
    return '$_temp0';
  }

  @override
  String partCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个部分',
      one: '1 个部分',
    );
    return '$_temp0';
  }

  @override
  String inMeetings(Object parts, Object meetings) {
    return '$parts，分布在 $meetings';
  }

  @override
  String assignedVoicesTo(int count, Object name) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已将 $count 个声音分配给 $name',
      one: '已分配给 $name',
    );
    return '$_temp0';
  }

  @override
  String addSharedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '将 $count 个分享的文件添加到项目',
      one: '将分享的文件添加到项目',
    );
    return '$_temp0';
  }

  @override
  String keepSpeechSaved(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已保存。语音现在最多等待 $count 天以便合并。',
      one: '已保存。语音现在最多等待 1 天以便合并。',
    );
    return '$_temp0';
  }

  @override
  String titlesCanBeAtMost(Object max) {
    return '标题最多 $max 个字符。';
  }

  @override
  String promptUnknownPlaceholder(Object example) {
    return '提示词中有未知或未闭合的 $example。';
  }

  @override
  String get live => '● 在线';

  @override
  String get offline => '○ 离线';

  @override
  String minutesShort(Object minutes) {
    return '$minutes 分钟';
  }

  @override
  String get appLanguage => '应用语言';

  @override
  String get appLanguageDescription => 'Koalino 菜单和消息的语言。';

  @override
  String get systemLanguage => '跟随系统';

  @override
  String photosAddedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已添加 $count 张照片',
      one: '已添加 1 张照片',
    );
    return '$_temp0';
  }

  @override
  String moreModels(int count) {
    return '更多模型（$count）';
  }

  @override
  String get moreModelsHint => '更小但错误稍多的版本、仅支持英语的版本，以及可能超出此设备承受能力的模型。';

  @override
  String get microphoneSilencedByCall =>
      '另一个应用正在通话中使用麦克风，Android 已将其静音：通话结束前不会录到任何内容。';

  @override
  String get handoffInTheBrowser =>
      'Koalino 会在浏览器中打开网站（你已用自己的账号登录），并把会议连同你的提示交给它。下一个窗口会说明在那里要做什么。';

  @override
  String toKeepTheSummaryCopyTheWebsite(String appName) {
    return '要保存摘要，请在网站上复制 $appName 的回答，然后回到 Koalino。';
  }

  @override
  String get useYourClaudeOrChatgptPlan => '使用你的 Claude 或 ChatGPT 套餐';

  @override
  String get summariesCanUseYourOwnPlan =>
      '摘要可以使用你自己的 Claude 或 ChatGPT 订阅，无需 API 密钥。安装 Claude Code 或 Codex 并登录一次，之后 Koalino 会自动撰写摘要。';

  @override
  String get installClaudeCode => '安装 Claude Code';

  @override
  String get forAClaudePlanOpensTheGuide => '适用于 Claude 套餐。打开安装指南。';

  @override
  String get installCodex => '安装 Codex';

  @override
  String get forAChatgptPlanOpensTheGuide => '适用于 ChatGPT 套餐。打开安装指南。';

  @override
  String get skipUseTheWebsite => '跳过：使用网站';

  @override
  String get koalinoCopiesTheMeetingAndOpens =>
      '无需安装：Koalino 复制会议并打开 claude.ai 或 chatgpt.com，你在那里粘贴即可。';

  @override
  String get addAConnection => '添加连接';

  @override
  String get claudeCodeOrCodexAlreadyInstalled =>
      '已安装 Claude Code 或 Codex，或你网络中的服务器。';

  @override
  String get noConnectionNeededUseTheWebsite =>
      '无需连接。在会议中打开“摘要”，选择“使用你自己的 AI 应用…”：Koalino 会复制会议并打开 claude.ai 或 chatgpt.com。';

  @override
  String get afterInstallingAddTheConnection =>
      '安装并登录后，回到这里选择“添加连接 → 添加连接 → Claude Code 或 Codex”。';

  @override
  String get claudeAiOrChatgptComInYourBrowser =>
      '在浏览器中使用 claude.ai 或 chatgpt.com，使用你的账号';

  @override
  String get newMeeting => '新建会议';

  @override
  String get newMeetingExplanation => '没有录音的会议。写笔记或添加文件，摘要和聊天会用到它们。';

  @override
  String get titleOptional => '标题（可选）';

  @override
  String handoffStepPressEnter(String site) {
    return '$site 打开时会议已在消息框中。按 Enter 发送。';
  }

  @override
  String handoffStepSentByItself(String site) {
    return '$site 打开后会自动发送会议。如有提示，请先登录。';
  }

  @override
  String handoffStepPaste(String site) {
    return '$site 会打开，Koalino 会复制会议。点击消息框，粘贴（Ctrl+V）后按 Enter。';
  }

  @override
  String get handoffStepDragFiles => '同时会打开一个包含这些文件的文件夹：把它们全部拖进消息框，以文件形式发送。';

  @override
  String get handoffStepCopyAnswer => '回答完成后，点击其下方的复制按钮。';

  @override
  String get handoffStepComeBack => '回到 Koalino：回答会成为会议摘要，可撤销。';

  @override
  String get handoffStepAskThere => '然后在那里提出关于会议的问题。';

  @override
  String openSite(String site) {
    return '打开 $site';
  }

  @override
  String writtenBySourceOn(String source, String createdAt) {
    return '由 $source 于 $createdAt 撰写';
  }

  @override
  String get noRecordingWriteNotesOrAttach => '没有录音。写笔记或附加文件，摘要和聊天会用到它们。';

  @override
  String handoffStepOpensFilled(String site) {
    return '$site 打开时会议已在消息框中。';
  }

  @override
  String handoffStepOpensCopied(String site) {
    return '$site 会打开，Koalino 会复制会议。';
  }

  @override
  String get handoffStepSend => '按 Enter 发送。';

  @override
  String get handoffStepPasteSend => '点击消息框，粘贴会议（Ctrl+V）后按 Enter。';

  @override
  String get chatHandoffIntro =>
      '用你自己的账号，在你自己的 AI 应用里提问。Koalino 会把笔记和转写交给它，照片和文件以文件形式一并发送。';

  @override
  String get plainTextFile => '纯文本 (.txt)';

  @override
  String get unableToExportTheSummary => '无法导出摘要';

  @override
  String get addAVoice => '添加声音';

  @override
  String alreadySavedVoiceAddsToThem(String name) {
    return '$name 已保存：此声音将添加给此人。';
  }

  @override
  String addVoicesTo(String name) {
    return '为 $name 添加声音';
  }

  @override
  String tickTheVoicesThatAre(String name) {
    return '这些声音未被识别为任何人。勾选属于 $name 的声音；可用“播放”确认。';
  }

  @override
  String addTo(String name) {
    return '添加给 $name';
  }

  @override
  String get apiKey => 'API 密钥';

  @override
  String get onlyForOnlineServicesKeptIn => '仅用于在线服务。保存在此设备的钥匙串中，只发送到此地址。';

  @override
  String get aKeyIsSavedLeaveEmpty => '已保存密钥。留空即保留。';

  @override
  String get removeTheSavedKey => '删除已保存的密钥';

  @override
  String get presetOllama => '在你的电脑或网络上：免费、私密、可离线使用（Gemma、Qwen、Llama…）。';

  @override
  String get presetLmStudio => '与 Ollama 相同，带有选择模型的桌面应用。';

  @override
  String get presetGemini =>
      'Google。免费层级有限额；在免费层级 Google 可能使用你发送的内容改进其产品。密钥：aistudio.google.com。';

  @override
  String get presetDeepseek => '按用量付费，非常便宜，预付额度。密钥：platform.deepseek.com。';

  @override
  String get presetQwen => '阿里巴巴。按用量付费。密钥：阿里云百炼 (Model Studio)。';

  @override
  String get presetKimi => 'Moonshot AI。按用量付费。密钥：platform.moonshot.ai。';

  @override
  String get presetMistral => 'Mistral（法国）。免费层级限额严格。密钥：console.mistral.ai。';

  @override
  String get presetOpenrouter =>
      '一个密钥用于多种模型（Gemini、DeepSeek、Qwen…），部分免费；文本也会经过 OpenRouter。密钥：openrouter.ai。';

  @override
  String get keepMeetingRecordings => '保留会议录音';

  @override
  String get keepMeetingRecordingsSubtitle =>
      '转写后保留每场会议的音频，加密存放在此设备上，以便回放。默认关闭。';

  @override
  String get keepMeetingRecordingsQuestion => '保留会议录音？';

  @override
  String get keepRecordingsWarning =>
      '录音包含会议中每个人的声音。请告知被录音的人，并遵守当地关于同意的规定：在许多地方，录下他人需要其许可。录音加密保存在此设备上，不会同步；每 10 分钟语音约占 2 MB。录音会随会议一起删除，或用“清除所有录音”一次全部删除。只保留语音，因此静音部分会以静音播放。';

  @override
  String get keepRecordings => '保留录音';

  @override
  String recordingsKept(int count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已保留 $count 段录音 · $size MB',
      zero: '未保留录音',
    );
    return '$_temp0';
  }

  @override
  String get wipeAllRecordings => '清除所有录音';

  @override
  String get wipeAllRecordingsQuestion => '清除所有录音？';

  @override
  String get wipeAllRecordingsExplanation =>
      '此设备上保留的所有会议录音都将被删除。转写和笔记会保留。此操作无法撤销。';

  @override
  String recordingsWiped(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '已清除 $count 段录音。',
      zero: '没有可清除的录音。',
    );
    return '$_temp0';
  }

  @override
  String get recordingsUnavailable => '目前无法访问录音。';

  @override
  String get playRecording => '播放录音';

  @override
  String get stopRecordingPlayback => '停止播放';

  @override
  String get recordingCouldNotBePlayed => '无法播放该录音。';

  @override
  String get columnWidth => '列宽';

  @override
  String get subprojects => '子项目';

  @override
  String get newSubproject => '新建子项目';

  @override
  String get moveProjectTo => '移动到…';

  @override
  String get topLevel => '顶层';

  @override
  String moveProjectTitle(String project) {
    return '将 $project 移动到';
  }

  @override
  String get movedToTopLevel => '已移动到顶层';

  @override
  String get theProjectCouldNotBeMoved => '无法移动该项目。';

  @override
  String subprojectCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 个子项目',
    );
    return '$_temp0';
  }

  @override
  String get chooseAProject => '选择一个项目以查看其会议和文件。';

  @override
  String get chooseAMeetingOrFile => '选择一个会议或文件以在此查看。';

  @override
  String get backgroundPassTitle => '在 Koalino 关闭时也改进转写';

  @override
  String get backgroundPassAllowed =>
      '已允许：即使 Koalino 已关闭或开启了省电模式，手机充电时也会改进待处理的转写。';

  @override
  String get backgroundPassAsk =>
      '点按以允许 Android 在手机充电时执行，即使开启了省电模式。否则只会在 Koalino 打开时执行。';

  @override
  String get chatPrompt => '聊天提示词';

  @override
  String chatPromptHelp(String placeholder) {
    return '$placeholder 表示你提问的对象：一个会议、一个项目或你的全部笔记。回答始终用编号注明出处，你的笔记附在提示词之后。';
  }

  @override
  String get saveChatPrompt => '保存聊天提示词';

  @override
  String get chatPromptSaved => '聊天提示词已保存。';

  @override
  String get theBuiltInChatPromptIsBack => '已恢复内置的聊天提示词。';

  @override
  String get promptsSharedWithPairedDevices =>
      '两个提示词在你所有已配对的设备上都相同：在这里所做的更改会在它们连接时同步过去。';

  @override
  String get deviceOnline => '在线';

  @override
  String get deviceOffline => '离线';

  @override
  String get neverSynced => '从未同步';

  @override
  String get syncedJustNow => '刚刚同步';

  @override
  String lastSyncedMinutesAgo(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '上次同步：$count 分钟前',
    );
    return '$_temp0';
  }

  @override
  String lastSyncedHoursAgo(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '上次同步：$count 小时前',
    );
    return '$_temp0';
  }

  @override
  String lastSyncedOn(String date) {
    return '上次同步：$date';
  }

  @override
  String devicesOnlineCount(int online, int total) {
    return '$online/$total 台在线';
  }

  @override
  String get microphoneSeemsMutedOrOff =>
      '麦克风似乎已静音或关闭：没有收到任何声音。请检查静音开关或按键，并确认系统设置中已开启麦克风访问权限。';

  @override
  String get windowsBlocksTheMicrophone =>
      'Windows 正在阻止应用使用麦克风：没有收到任何声音。请在“设置 → 隐私和安全性 → 麦克风”中开启“麦克风访问权限”和“允许桌面应用访问你的麦克风”。';

  @override
  String get backUpAllData => '备份所有数据';

  @override
  String get backUpAllDataDescription => '将会议、笔记、附件和保留的录音保存到一个加密文件中';

  @override
  String get restoreFromBackup => '从备份恢复';

  @override
  String get restoreFromBackupDescription => '用备份替换此设备上的数据';

  @override
  String get backupPassphraseTitle => '备份密码短语';

  @override
  String get backupPassphraseExplanation =>
      '备份使用此密码短语加密。没有它，任何人（包括你自己）都无法打开备份，请妥善保管。';

  @override
  String get backupPassphraseLabel => '密码短语';

  @override
  String get repeatPassphraseLabel => '再次输入密码短语';

  @override
  String get passphraseTooShort => '请至少使用 8 个字符。';

  @override
  String get passphrasesDoNotMatch => '两次输入的密码短语不一致。';

  @override
  String get continueAction => '继续';

  @override
  String get backingUp => '正在备份…';

  @override
  String get restoringBackup => '正在恢复…';

  @override
  String backupSaved(int files, String size) {
    return '备份已保存：$files 个文件，$size MB。';
  }

  @override
  String get restoreConfirmTitle => '替换此设备上的数据？';

  @override
  String get restoreConfirmBody =>
      '此设备上的会议和笔记将被备份中的内容替换。当前数据会另行保留，直到下一次恢复。之后 Koalino 会重新启动。';

  @override
  String get restoreAction => '恢复';

  @override
  String restoreReady(int files) {
    return '已恢复 $files 个文件。Koalino 现在将重新启动以打开它们。';
  }

  @override
  String get restartNow => '重新启动';

  @override
  String get backupWrongPassphrase => '密码短语错误。';

  @override
  String get backupNotABackup => '此文件不是 Koalino 备份。';

  @override
  String get backupTooNew => '此备份由较新版本的 Koalino 创建。请先更新应用。';

  @override
  String get backupDamaged => '备份已损坏或不完整。';

  @override
  String get backupFailed => '无法完成操作。请检查可用空间后重试。';

  @override
  String get automaticBackups => '自动备份';

  @override
  String get automaticBackupsDescription =>
      '每天在充电时保存到你选择的文件夹，也可以是云端（Google Drive、Nextcloud、OneDrive…）。离开手机前即已加密。';

  @override
  String automaticBackupFolder(String folder) {
    return '文件夹：$folder';
  }

  @override
  String get noAutomaticBackupYet => '尚无自动备份。';

  @override
  String lastAutomaticBackup(String time) {
    return '上次备份：$time';
  }

  @override
  String lastAutomaticBackupFailed(String time) {
    return '$time 的备份失败。请检查文件夹。';
  }

  @override
  String get backUpNow => '立即备份';

  @override
  String get backupFolderUnavailable => '无法写入备份文件夹。请确认它仍然存在且其应用已登录，或选择其他文件夹。';

  @override
  String get automaticBackupsNotSetUp => '需要重新设置自动备份：请将其关闭后再打开。';

  @override
  String get renameSpeaker => '重命名说话人';

  @override
  String get nameThisVoice => '为这个声音命名';

  @override
  String get doubleClickOrLongPressToRename => '双击或长按以重命名';

  @override
  String get theNewNameAppliesToEveryPassage =>
      '新名称会应用到这个声音的每一段发言，包括你所有的会议和已配对的设备。';

  @override
  String anotherSavedPersonIsCalled(Object name) {
    return '已有另一位保存的人叫“$name”。如果是同一个人，请将两者合并。';
  }

  @override
  String get useOneTo100CharactersOnOneLine => '请在一行内输入 1 到 100 个字符。';

  @override
  String versionNumber(Object version) {
    return '版本 $version';
  }

  @override
  String get developerSettings => '开发者设置';

  @override
  String get appVariablesForExpertsAndDevelopers => '面向专家和开发者的应用变量';

  @override
  String tapsLeftToTurnOnDeveloperSettings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '再点按 $count 次即可开启开发者设置',
    );
    return '$_temp0';
  }

  @override
  String get developerSettingsAreOn => '开发者设置已开启';

  @override
  String get developerSettingsWarning =>
      '这些变量会改变 Koalino 的内部工作方式。错误的值可能导致转写、同步或浏览器扩展停止工作。保存的值在 Koalino 重启后生效。';

  @override
  String get restartKoalinoToApply => '重启 Koalino 以应用保存的值。';

  @override
  String nowAndDefault(Object now, Object defaultValue) {
    return '当前：$now · 默认：$defaultValue';
  }

  @override
  String get notSet => '未设置';

  @override
  String savedForTheNextStart(Object value) {
    return '已保存，下次启动生效：$value';
  }

  @override
  String get useDefault => '使用默认值';

  @override
  String get resetAll => '全部重置';

  @override
  String get turnOffDeveloperSettings => '关闭开发者设置';

  @override
  String get turnOffDeveloperSettingsExplanation =>
      '下次启动时所有值都会恢复为默认值，此部分将再次隐藏。';

  @override
  String get turnOff => '关闭';

  @override
  String mergeIntoAnotherPerson(Object name) {
    return '将 $name 合并到另一个人';
  }

  @override
  String mergeNameInto(Object name) {
    return '将 $name 合并到…';
  }

  @override
  String mergeWarning(Object source, Object target) {
    return '$source 的所有发言和声音样本都会移到 $target，$source 将被删除，已配对的设备上也一样。此操作无法撤销。';
  }

  @override
  String get merge => '合并';

  @override
  String mergeWithName(Object name) {
    return '与 $name 合并';
  }

  @override
  String mergedInto(Object source, Object target) {
    return '$source 已并入 $target';
  }

  @override
  String get thesePeopleCouldNotBeMerged => '无法合并这些人，请重试。';

  @override
  String get peopleCannotBeMergedWhileRecording => '录制会议时无法合并人员。';

  @override
  String get notEnoughFreeSpaceToDownload => '可用空间暂不足以下载它。';

  @override
  String get downloadTheBestModelByItself => '自动下载最佳模型';

  @override
  String get downloadTheBestModelByItselfHint =>
      '连接 Wi-Fi 且空间足够时，启动时会下载“自动”为此设备选择的模型。在此期间，录音使用当前模型。';

  @override
  String gettingAMoreAccurateModel(String name, String percent) {
    return '正在为此设备获取更准确的模型（$name）：$percent';
  }

  @override
  String get recordingsUseTheCurrentModelMeanwhile => '在它准备好之前，录音继续使用当前模型。';

  @override
  String get youCanRecordOnceItIsReady => '准备好后即可录音。';
}
