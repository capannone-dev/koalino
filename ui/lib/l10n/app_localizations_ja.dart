// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Japanese (`ja`).
class AppLocalizationsJa extends AppLocalizations {
  AppLocalizationsJa([String locale = 'ja']) : super(locale);

  @override
  String get unableToCheckTheAppLock => 'アプリロックを確認できませんでした。もう一度お試しください。';

  @override
  String get unableToShowTheUnlockPrompt => 'ロック解除の画面を表示できませんでした。もう一度お試しください。';

  @override
  String get setAScreenLockOnThis => '先にこのデバイスで画面ロックを設定してください。';

  @override
  String get unableToChangeTheAppLock => 'アプリロックを変更できませんでした。もう一度お試しください。';

  @override
  String get aCallApp => '通話アプリ';

  @override
  String get call => '通話';

  @override
  String call2(Object app) {
    return '$app の通話';
  }

  @override
  String theCallEndedSoTheRecording(Object tiedApp) {
    return '$tiedApp の通話が終了したため、録音を停止しました。';
  }

  @override
  String get restartYourBrowserAndCallApps =>
      '通話中の話者を Koalino が読み取れるよう、ブラウザーと通話アプリを一度再起動してください。';

  @override
  String get unableToLoadDevices => 'デバイスを読み込めませんでした';

  @override
  String get thisDeviceIsNotOnA =>
      'このデバイスはまだネットワークに接続されていません。接続してからもう一度お試しください。';

  @override
  String get thatDidNotWorkTryAgain => 'うまくいきませんでした。もう一度お試しください。';

  @override
  String get use1To64CharactersWithout => '改行を含まない 1〜64 文字にしてください。';

  @override
  String get theNameCouldNotBeSaved => '名前を保存できませんでした。もう一度お試しください。';

  @override
  String get thisInviteHasExpiredMakeA => 'この招待は期限切れです。もう一方のデバイスで新しく作成してください。';

  @override
  String get thisInviteWasMadeOnThis =>
      'この招待はこのデバイスで作成されたものです。もう一方のデバイスで使ってください。';

  @override
  String get thatDeviceIsAlreadyPaired => 'そのデバイスはすでにペアリングされています。';

  @override
  String get thatIsNotAValidInvite => '有効な招待ではありません。もう一度コピーしてください。';

  @override
  String get deviceSyncIsNotRunning => 'デバイス同期が実行されていません。';

  @override
  String get theOtherDeviceIsNoLonger => 'もう一方のデバイスはペアリングを終了しました。もう一度お試しください。';

  @override
  String get theOtherAppCouldNotBe => 'もう一方のアプリを開けませんでした。';

  @override
  String get theOtherApp => 'もう一方のアプリ';

  @override
  String app(Object displayName) {
    return '$displayName アプリ';
  }

  @override
  String get unableToChangeTheLogLevel => 'ログレベルを変更できませんでした';

  @override
  String logsExportedTo(Object filePath) {
    return 'ログを $filePath に書き出しました';
  }

  @override
  String get unableToShareTheLogs => 'ログを共有できませんでした';

  @override
  String get noLogsToExportYet => '書き出すログはまだありません';

  @override
  String get unableToExportTheLogs => 'ログを書き出せませんでした';

  @override
  String couldNotBeAttached(Object attached, Object failedPaths) {
    return '$attached。$failedPaths 件は添付できませんでした';
  }

  @override
  String get theFilesCouldNotBeAttached => 'ファイルを添付できませんでした。';

  @override
  String get unableToShareTheExport => '書き出したファイルを共有できませんでした';

  @override
  String exportedTo(Object filePath) {
    return '$filePath に書き出しました';
  }

  @override
  String get exportSaved => '書き出しを保存しました';

  @override
  String get unableToSaveTheExport => '書き出しを保存できませんでした';

  @override
  String get thisMeetingNoLongerExists => 'この会議はもう存在しません';

  @override
  String get exportIsUnavailableUntilTheApp => 'アプリの起動が完了するまで書き出しはできません';

  @override
  String get unableToExportTheMeeting => '会議を書き出せませんでした';

  @override
  String get unableToLoadMeetings => '会議を読み込めませんでした';

  @override
  String get unableToLoadThisMeeting => 'この会議を読み込めませんでした';

  @override
  String get enterATitle => 'タイトルを入力してください。';

  @override
  String titlesCanBeAtMostCharacters(
      Object maxMeetingTitleCharacters, Object runes) {
    return 'タイトルは最大 $maxMeetingTitleCharacters 文字です（現在 $runes 文字）。';
  }

  @override
  String get titlesCanTContainLineBreaks => 'タイトルに改行や制御文字は使えません。';

  @override
  String get notPairedYetMakeTheRelay =>
      'まだペアリングされていません。リレーを検出可能にしてから、「デバイス」の「近くのデバイス」でペアリングしてください。';

  @override
  String get connectingToTheRelay => 'リレーに接続中…';

  @override
  String get connectedToTheRelayAskingIt => 'リレーに接続しました。接続の受け入れを依頼しています…';

  @override
  String get connectedYourOtherDevicesCanReach =>
      '接続しました。他のデバイスからリレー経由でこのデバイスに接続できます。';

  @override
  String get savedThisAppliesNowWithNo => '保存しました。再起動なしですぐに反映されます。';

  @override
  String get thatRelayAddressIsNotValid =>
      'そのリレーのアドレスは無効です。/ip4/203.0.113.5/tcp/4001/p2p/12D3Koo… のような形式です';

  @override
  String get aDeviceAddressIsNotValid =>
      '無効なデバイスアドレスがあります。いずれも /p2p/ とデバイス ID で終わります。';

  @override
  String get unableToSaveTheAddresses => 'アドレスを保存できませんでした。';

  @override
  String get thePhotoCouldNotBeSaved => '写真を保存できませんでした。';

  @override
  String get enterAName => '名前を入力してください。';

  @override
  String namesCanBeAtMostCharacters(Object maxMeetingTitleCharacters) {
    return '名前は最大 $maxMeetingTitleCharacters 文字です。';
  }

  @override
  String get namesCanTContainLineBreaks => '名前に改行や制御文字は使えません。';

  @override
  String get unableToLoadProjects => 'プロジェクトを読み込めませんでした';

  @override
  String get unableToLoadThisProject => 'このプロジェクトを読み込めませんでした';

  @override
  String get theFilesCouldNotBeAdded => 'ファイルを追加できませんでした。';

  @override
  String couldNotBeAdded(Object failedPaths) {
    return '$failedPaths 件は追加できませんでした';
  }

  @override
  String get unableToStartRecording => '録音を開始できませんでした';

  @override
  String get unableToPauseRecording => '録音を一時停止できませんでした';

  @override
  String get unableToResumeRecording => '録音を再開できませんでした';

  @override
  String get unableToStopRecording => '録音を停止できませんでした';

  @override
  String get recordingSaved => '録音を保存しました';

  @override
  String get microphoneAccessIsNeededToRecord => '録音にはマイクへのアクセスが必要です';

  @override
  String get keepTheAppOpenAndroidDid =>
      'アプリを開いたままにしてください。Android がバックグラウンドでの録音を許可しませんでした';

  @override
  String get unableToOpenTheMicrophone => 'マイクを開けませんでした';

  @override
  String saved(Object label) {
    return '$label を保存しました。';
  }

  @override
  String removed(Object label) {
    return '$label を削除しました。';
  }

  @override
  String isNowTheDefault(Object id) {
    return '$id がデフォルトになりました。';
  }

  @override
  String answered(Object id, Object reply) {
    return '$id の応答: $reply';
  }

  @override
  String get promptSaved => 'プロンプトを保存しました。';

  @override
  String get theBuiltInPromptIsBack => '組み込みのプロンプトに戻しました。';

  @override
  String get finishedMeetingsWillBeSummarizedAutomatically =>
      '終了した会議は自動的に要約されます。';

  @override
  String get automaticSummariesAreOff => '自動要約はオフです。';

  @override
  String get thisComputerNoLongerWritesSummaries =>
      'このコンピューターは、他のデバイスのための要約作成やチャットへの回答を行わなくなりました。';

  @override
  String nowWritesTheSummariesAndAnswers(Object id) {
    return '$id が、他のデバイスから依頼された要約を作成し、チャットに回答します。';
  }

  @override
  String get requestCancelled => 'リクエストをキャンセルしました。';

  @override
  String get thisMeetingUsesTheSettingsPrompt => 'この会議では再び設定のプロンプトを使います。';

  @override
  String get promptSavedForThisMeeting => 'この会議のプロンプトを保存しました。';

  @override
  String get summarySaved => '要約を保存しました。';

  @override
  String get theConnection => '接続';

  @override
  String get summaryWritten => '要約を作成しました。';

  @override
  String theTranscriptWasReadInParts(Object parts) {
    return '文字起こしは $parts 部に分けて読み込みました。';
  }

  @override
  String leftOut(Object sent, Object imagesSkipped) {
    return '$sent、$imagesSkipped 枚は除外しました。';
  }

  @override
  String get theModelStoppedBeforeTheEnd => 'モデルが最後まで出力する前に停止しました。';

  @override
  String get waitingForYourComputerItWrites =>
      'コンピューターを待っています。電源が入り、このデバイスに接続され、「設定 → AI 接続」で応答するよう設定されていれば要約を作成します。';

  @override
  String get yourComputerIsWritingTheSummary => 'コンピューターが要約を作成中…';

  @override
  String yourComputerCouldNotWriteThe(Object message) {
    return 'コンピューターが要約を作成できませんでした: $message';
  }

  @override
  String get unableToRefreshTelemetry => 'テレメトリーを更新できませんでした';

  @override
  String get automatic => '自動';

  @override
  String get transcriptionLanguage => '文字起こしの言語';

  @override
  String get transcriptionLanguages => '文字起こしの言語';

  @override
  String get theLanguageCouldNotBeChanged => '言語を変更できませんでした。';

  @override
  String get thisDevice => 'このデバイス';

  @override
  String get bestAvailable => '最適なもの';

  @override
  String get aRemovedDevice => '削除されたデバイス';

  @override
  String get theTranscriptionDeviceCouldNotBe =>
      '文字起こしに使うデバイスを変更できませんでした。まだペアリングされているか確認してください。';

  @override
  String get downloadFinishedYouCanNowChoose => 'ダウンロードが完了しました。このモデルを選択できます。';

  @override
  String get downloadCancelled => 'ダウンロードをキャンセルしました。';

  @override
  String get theDownloadStoppedNoConnectionTo =>
      'ダウンロードが停止しました: サーバーに接続できません。現在のモデルは引き続き使えます。';

  @override
  String get theDownloadedFileFailedItsCheck =>
      'ダウンロードしたファイルが検証に失敗したため削除しました。現在のモデルは引き続き使えます。';

  @override
  String get theModelCouldNotBeSaved =>
      'モデルを保存できませんでした。空き容量を確認してください。現在のモデルは引き続き使えます。';

  @override
  String get theDownloadServerRefusedTheFile =>
      'ダウンロードサーバーがファイルを拒否しました。現在のモデルは引き続き使えます。';

  @override
  String get savedTheNextRecordingUsesThis => '保存しました。次回の録音からこのモデルを使います。';

  @override
  String get savedTheNextRecordingUsesThis2 => '保存しました。次回の録音から使われます。';

  @override
  String get savedTheNextRecordingUsesThe => '保存しました。次回の録音では自動選択を使います。';

  @override
  String get savedMeetingsRecordedFromNowOn => '保存しました。今後録音する会議で使われます。';

  @override
  String get savedTheNextRecordingUsesThese => '保存しました。次回の録音からこれらの名前と用語を使います。';

  @override
  String get eachTermCanHaveAtMost => '各用語は最大 60 文字、全体で最大 500 文字です。';

  @override
  String get chooseAWindowBetween1And => '1〜60 日の期間を選んでください。';

  @override
  String get theDownloadCouldNotStartPlease => 'ダウンロードを開始できませんでした。もう一度お試しください。';

  @override
  String get savedTheNextRecordingIsTranscribed =>
      '保存しました。次回の録音は NPU で文字起こしします。';

  @override
  String get savedTheNextRecordingIsTranscribed2 =>
      '保存しました。次回の録音はプロセッサーで文字起こしします。';

  @override
  String get theNpuFilesAreNotAll => 'NPU 用ファイルのダウンロードがまだ完了していません。';

  @override
  String get theNpuFilesWereRemoved => 'NPU 用ファイルを削除しました。';

  @override
  String get theNpuFilesCouldNotBe => 'NPU 用ファイルを削除できませんでした。もう一度お試しください。';

  @override
  String get downloadedRecordingsNowTranscribeOnThe =>
      'ダウンロードしました。録音は NPU で文字起こしされます。';

  @override
  String get theDownloadStoppedNoConnectionTo2 =>
      'ダウンロードが停止しました: サーバーに接続できません。';

  @override
  String get aDownloadedFileFailedItsCheck =>
      'ダウンロードしたファイルの一つが検証に失敗したため削除しました。';

  @override
  String get theFilesCouldNotBeSaved =>
      'ファイルを保存できませんでした。空き容量を確認してください（約 800 MB 必要です）。';

  @override
  String get theDownloadServerRefusedTheFiles => 'ダウンロードサーバーがファイルを拒否しました。';

  @override
  String get theNextRecordingTriesTheGraphics => '次回の録音でグラフィックスチップを再度試します。';

  @override
  String get thatModelWonTBeProposed => 'そのモデルは今後提案されません。';

  @override
  String get modelRemoved => 'モデルを削除しました。';

  @override
  String get anotherModelIsDownloadingWaitFor => '別のモデルをダウンロード中です。完了までお待ちください。';

  @override
  String get thereIsnTEnoughFreeSpace => 'このモデルに必要な空き容量がありません。';

  @override
  String get thatDidnTWorkPleaseTry => 'うまくいきませんでした。もう一度お試しください。';

  @override
  String get unableToLoadVoiceGallery => '声のギャラリーを読み込めませんでした';

  @override
  String get unableToPlayThisVoice => 'この声を再生できませんでした';

  @override
  String get unableToDeleteThisVoice => 'この声を削除できませんでした';

  @override
  String get voiceDeleted => '声を削除しました';

  @override
  String get unableToDeleteThisPerson => 'この人物を削除できませんでした';

  @override
  String deleted(Object displayName) {
    return '$displayName を削除しました';
  }

  @override
  String get enterANameBeforeAssigning => '割り当てる前に名前を入力してください';

  @override
  String assignedTo(Object identityName) {
    return '$identityName に割り当てました';
  }

  @override
  String assignedToThereWasNotEnough(Object identityName) {
    return '$identityName に割り当てました。明瞭な発話が足りず、今後の録音でこの声を認識できません。';
  }

  @override
  String get unableToAssignTheSpeakerCluster => '話者グループを割り当てられませんでした';

  @override
  String get chooseAtLeastOneVoiceTo => '名前を付ける声を 1 つ以上選んでください';

  @override
  String thereWasNotEnoughClearSpeech(Object assigned) {
    return '$assigned。明瞭な発話が足りず、今後の録音でこの声を認識できません。';
  }

  @override
  String get theSettingCouldNotBeSaved => '設定を保存できませんでした。';

  @override
  String get theAppIsStillStartingTry => 'アプリはまだ起動中です。もう一度お試しください。';

  @override
  String get somethingWentWrongTryAgain => '問題が発生しました。もう一度お試しください。';

  @override
  String isUsingTheMicrophone(Object app) {
    return '$app がマイクを使用しています';
  }

  @override
  String get recordThisCall => 'この通話を録音しますか？';

  @override
  String theAiServerRejectedTheRequest(Object detail) {
    return 'AI サーバーがリクエストを拒否しました。\nサーバーの応答: $detail';
  }

  @override
  String claudeCodeOrCodexCouldNot(Object detail) {
    return 'Claude Code または Codex が応答できませんでした。\n応答内容: $detail';
  }

  @override
  String get typeAQuestion => '質問を入力してください。';

  @override
  String get theQuestionIsTooLong => '質問が長すぎます。';

  @override
  String get addAnAiConnectionInSettings => '設定で AI 接続を追加するか、デフォルトの接続を選んでください。';

  @override
  String get thatAiConnectionNoLongerExists => 'その AI 接続はもう存在しません。';

  @override
  String get thereIsNothingToTalkAbout =>
      'まだ話題にできるものがありません。文字起こし、要約、メモ、ファイルのいずれもありません。';

  @override
  String get theAiConnectionSContextIs =>
      'AI 接続のコンテキストが小さすぎます。より大きなコンテキストの接続を選んでください。';

  @override
  String get thatMeetingNoLongerExists => 'その会議はもう存在しません。';

  @override
  String get thatProjectNoLongerExists => 'そのプロジェクトはもう存在しません。';

  @override
  String get theAiServerCouldNotBe => 'AI サーバーに接続できませんでした。接続を確認してください。';

  @override
  String get theAiServerRefusedTheApi => 'AI サーバーが API キーを拒否しました。';

  @override
  String get theAiServerIsBusyTry => 'AI サーバーが混雑しています。後でもう一度お試しください。';

  @override
  String get theModelDeclinedToAnswer => 'モデルが回答を拒否しました。';

  @override
  String get claudeCodeOrCodexIsNot =>
      'このコンピューターに Claude Code または Codex がインストールされていないか、アプリが見つけられません。';

  @override
  String get theNotesCouldNotBeRead => 'メモを読み込めませんでした。';

  @override
  String get noPairedComputerAnsweredOnA =>
      'ペアリング済みのコンピューターから応答がありません。コンピューターで「設定 → AI」を開き、「他のデバイス向けの要約」で接続を選んで、電源を入れたまま接続しておいてください。';

  @override
  String get theComputerTookTooLongTo => 'コンピューターの応答に時間がかかりすぎました。';

  @override
  String get theComputerDisconnectedBeforeAnsweringTry =>
      'コンピューターが応答する前に切断されました。もう一度お試しください。';

  @override
  String get theComputerAnswersAtMost30 =>
      'コンピューターが回答できるのは 1 時間に最大 30 件です。後でもう一度お試しください。';

  @override
  String get theComputerIsAnsweringOtherQuestions =>
      'コンピューターは他の質問に回答中です。少ししてからもう一度お試しください。';

  @override
  String get pairedDevicesAreOffOnThis => 'このスマートフォンではペアリング済みデバイスがオフになっています。';

  @override
  String get theAiServerCouldNotAnswer => 'AI サーバーが回答できませんでした。もう一度お試しください。';

  @override
  String get exportMeeting => '会議を書き出す';

  @override
  String get openaiCompatibleServer => 'OpenAI 互換サーバー';

  @override
  String get localServerOllama => 'ローカルサーバー（Ollama）';

  @override
  String get claudeCodeClaudePlan => 'Claude Code（Claude プラン）';

  @override
  String get codexChatgptPlan => 'Codex（ChatGPT プラン）';

  @override
  String get checkTheAddressAndModelPlain =>
      'アドレスとモデルを確認してください。暗号化なしの http:// はこのデバイスかローカルネットワークでのみ使えます。';

  @override
  String get thatConnectionNoLongerExists => 'その接続はもう存在しません。';

  @override
  String get addAnAiConnectionInSettings2 => 'まず設定で AI 接続を追加してください。';

  @override
  String get thisMeetingHasNoTranscriptOr => 'この会議にはまだ文字起こしもメモもありません。';

  @override
  String get thisMeetingIsTooLongFor =>
      'この会議はこの接続には長すぎます。より大きなコンテキストの接続を選んでください。';

  @override
  String get theAiServerCouldNotBe2 => 'AI サーバーに接続できませんでした。';

  @override
  String get theAiServerRefusedAccessUse =>
      'AI サーバーがアクセスを拒否しました。接続の API キーを確認してください。';

  @override
  String get theAiServerIsBusyOr => 'AI サーバーが混雑しているか上限を超えています。後でもう一度お試しください。';

  @override
  String get theAiServerRejectedTheRequest2 =>
      'AI サーバーがリクエストを拒否しました。モデル名を確認してください。';

  @override
  String get theAiServerFailedTryAgain => 'AI サーバーでエラーが発生しました。もう一度お試しください。';

  @override
  String get theAiServerSAnswerCould => 'AI サーバーの応答を読み取れませんでした。';

  @override
  String get claudeCodeOrCodexCouldNot2 =>
      'Claude Code または Codex が応答できませんでした。サインインしているか確認してください。';

  @override
  String get theSettingsCouldNotBeSaved => '設定を保存できませんでした。';

  @override
  String get theMeetingCouldNotBeRead => '会議を読み込みまたは保存できませんでした。';

  @override
  String get error => 'エラー';

  @override
  String get warning => '警告';

  @override
  String get infoDefault => '情報（デフォルト）';

  @override
  String get debug => 'デバッグ';

  @override
  String get trace => 'トレース';

  @override
  String get thisDevice2 => 'このデバイス';

  @override
  String get anotherDevice => '別のデバイス';

  @override
  String notConnected(Object label) {
    return '$label（未接続）';
  }

  @override
  String itIsKeptEncryptedUntil(Object until) {
    return '$until まで暗号化して保管されます。';
  }

  @override
  String get recordedOnThisDeviceOnly => 'このデバイスでのみ録音されました。';

  @override
  String recordedOnThisDeviceOnlyA(Object until) {
    return 'このデバイスでのみ録音されました。別のデバイスで録音した同じ会議は $until まで統合できます。';
  }

  @override
  String combinesTheRecordingsNothingIsLeft(Object combiner) {
    return '$combiner が録音を統合します。ここから送るものはもうありません。';
  }

  @override
  String waitingForAudioFrom(Object names, Object kept) {
    return '$names からの音声を待っています。$kept';
  }

  @override
  String combinesTheRecordingsWaitingForAudio(Object combiner, Object names) {
    return '$combiner が録音を統合します。$names からの音声を待っています。';
  }

  @override
  String get audioFromEveryDeviceIsHere =>
      'すべてのデバイスの音声が揃いました。このデバイスが空いたときに文字起こしを統合します。';

  @override
  String thisDeviceSAudioStillHas(Object combiner, Object kept) {
    return 'このデバイスの音声はまだ $combiner に届いていません。$kept';
  }

  @override
  String get transcriptCombinedFromEveryDevice => 'すべてのデバイスの文字起こしを統合しました。';

  @override
  String notCombinedAudioFromDidNot(Object plainNames) {
    return '統合されていません: $plainNames からの音声が 3 週間以内に届きませんでした。';
  }

  @override
  String meeting(Object createdAt) {
    return '会議 $createdAt';
  }

  @override
  String get writeSomethingFirst => 'まず何か入力してください。';

  @override
  String get theTextIsTooLong => 'テキストが長すぎます。';

  @override
  String get thatPartOfTheTranscriptChanged =>
      'その部分の文字起こしはその間に変更されました。会議を開き直してください。';

  @override
  String get theMeetingCouldNotBeSaved => '会議を保存できませんでした。';

  @override
  String get chooseAFullFolderPathInside =>
      'ホームフォルダー内のフォルダーの完全なパスを選んでください（ホームフォルダーそのものは不可）。';

  @override
  String get theFolderCouldNotBeWritten =>
      'フォルダーに書き込めませんでした。存在するか、書き込み権限があるか確認してください。';

  @override
  String get theNotesCouldNotBeRead2 => 'メモを読み込みまたは保存できませんでした。';

  @override
  String get theInviteAnswerIsIncomplete => '招待への応答が不完全です。';

  @override
  String get theRenameAnswerIsIncomplete => '名前変更への応答が不完全です。';

  @override
  String get thisDeviceHasNoCamera => 'このデバイスにはカメラがありません。';

  @override
  String get theCameraIsNotOpen => 'カメラが開いていません。';

  @override
  String get title => 'タイトル';

  @override
  String get summary => '要約';

  @override
  String get notes => 'メモ';

  @override
  String get transcript => '文字起こし';

  @override
  String get projectName => 'プロジェクト名';

  @override
  String file(Object fileName) {
    return 'ファイル $fileName';
  }

  @override
  String get typeWhatToLookFor => '検索する内容を入力してください。';

  @override
  String get typeAtLeastTwoLetters => '2 文字以上入力してください。';

  @override
  String get theSearchIsTooLong => '検索語が長すぎます。';

  @override
  String get exportLogs => 'ログを書き出す';

  @override
  String about(Object appName) {
    return '$appName について';
  }

  @override
  String get versionLicenseAndTheSoftwareIt => 'バージョン、ライセンス、使用しているソフトウェア。';

  @override
  String remove(Object label) {
    return '$label を削除しますか？';
  }

  @override
  String get itsApiKeyIsDeletedFrom => 'API キーはこのデバイスから削除されます。作成済みの要約は残ります。';

  @override
  String get cancel => 'キャンセル';

  @override
  String get remove2 => '削除';

  @override
  String get aiConnections => 'AI 接続';

  @override
  String get addConnection => '接続を追加';

  @override
  String get summariesSendTheMeetingSTranscript =>
      '要約では、会議の文字起こし、メモ、写真を接続先のサーバーに送ります。ローカルネットワーク上のサーバーならデータは手元に留まります。';

  @override
  String get noConnectionsYet => '接続はまだありません';

  @override
  String get addAServerOnYourNetwork =>
      'ネットワーク上のサーバー（Ollama、LM Studio、llama.cpp）か、このコンピューターの Claude Code または Codex を追加してください。';

  @override
  String get summarizeMeetingsAutomatically => '会議を自動的に要約';

  @override
  String get withTheDefaultConnectionWhenA => '会議の文字起こしが確定したときに、デフォルトの接続で行います。';

  @override
  String get summariesForMyOtherDevices => '他のデバイス向けの要約';

  @override
  String get aPairedPhoneCanAskThis =>
      'ペアリング済みのスマートフォンから、このコンピューター上のメモのコピーをもとに、この接続で要約の作成やチャットへの回答を依頼できます。そのプランの利用量に含まれます。1 時間あたり最大 10 件の要約と 30 件のチャット回答です。';

  @override
  String get off => 'オフ';

  @override
  String get summaryPrompt => '要約のプロンプト';

  @override
  String placeholdersTheTranscriptIsAlwaysSent(Object placeholders) {
    return 'プレースホルダー: $placeholders。文字起こしは常にプロンプトの後に送られます。';
  }

  @override
  String get savePrompt => 'プロンプトを保存';

  @override
  String get resetToBuiltIn => '組み込みに戻す';

  @override
  String get defaultLabel => 'デフォルト';

  @override
  String get installed => 'インストール済み';

  @override
  String get notFoundOnThisComputer => 'このコンピューターに見つかりません';

  @override
  String get unencryptedHttpLocalNetworkOnly => '暗号化なしの http、ローカルネットワークのみ';

  @override
  String get runsWithYourPlanSSign => 'このコンピューターのプランのサインインで実行';

  @override
  String actionsFor(Object label) {
    return '$label の操作';
  }

  @override
  String get makeDefault => 'デフォルトにする';

  @override
  String get test => 'テスト';

  @override
  String get edit => '編集';

  @override
  String get enterANameAnAddressAnd => '名前、アドレス、モデルを入力してください。';

  @override
  String get theContextIsANumberOf => 'コンテキストはトークン数です（例: 8192）。';

  @override
  String edit2(Object label) {
    return '$label を編集';
  }

  @override
  String get name => '名前';

  @override
  String get protocol => 'プロトコル';

  @override
  String get runsTheToolInstalledAndSigned =>
      'このコンピューターにインストールされサインイン済みのツールを実行するため、API クレジットではなく Claude または ChatGPT のプランで要約を作成します。会議はそのアカウントで Anthropic または OpenAI に送られます。';

  @override
  String get address => 'アドレス';

  @override
  String get httpsAlwaysHttpOnlyOnThis =>
      'https:// は常に可。http:// はこのデバイスかローカルネットワークのみ。';

  @override
  String get model => 'モデル';

  @override
  String get leaveEmptyForTheToolS => '空欄にするとツールのデフォルトを使います。';

  @override
  String get contextTokens => 'コンテキスト（トークン）';

  @override
  String get saveConnection => '接続を保存';

  @override
  String get koalinoIsLocked => 'Koalino はロックされています';

  @override
  String get unlockWithYourFingerprintFaceOr => '指紋、顔、または画面ロックで解除してください。';

  @override
  String get unlock => 'ロック解除';

  @override
  String copied(Object what) {
    return '$what をコピーしました';
  }

  @override
  String get aiAssistantAccess => 'AI アシスタントのアクセス';

  @override
  String get letYourOwnAiAssistantSuch =>
      'Claude Code、Codex、Claude Desktop などの自分の AI アシスタントに、会議とプロジェクト（タイトル、要約、メモ、文字起こし、プロジェクトのファイル）の読み取りを許可します。読み取りのみで、変更や削除はできません。';

  @override
  String get whatTheAssistantReadsMayBe =>
      'アシスタントが読み取った内容は、そのサービスのあなたのアカウントでオンラインのモデルに送られることがあります。会議の内容を任せられるアシスタントにだけオンにしてください。';

  @override
  String get theAssistantServerIsNotInstalled =>
      'このアプリにはアシスタント用サーバーがインストールされていません。';

  @override
  String get letMyAiAssistantReadMy => 'AI アシスタントにメモの読み取りを許可';

  @override
  String get connectYourAssistant => 'アシスタントを接続';

  @override
  String get yourNotesAreUnlockedWithA =>
      'メモはパスフレーズで解除されます。パスフレーズはアシスタントの設定に書き込まれることはありません。サーバーがメモを読むのは、アシスタントがサーバーを起動する環境で NOTE_TAKER_ENCRYPTION_PASSPHRASE が設定されている場合だけです。';

  @override
  String get claudeCodeRunInATerminal => 'Claude Code: ターミナルで実行';

  @override
  String get command => 'コマンド';

  @override
  String get claudeDesktopAndOtherMcpClients =>
      'Claude Desktop など MCP クライアント: 各設定に追加';

  @override
  String get settings => '設定';

  @override
  String copy(Object title) {
    return 'コピー: $title';
  }

  @override
  String isUsingTheMicrophoneRecordThis(Object app) {
    return '$app がマイクを使用しています。この通話を録音しますか？';
  }

  @override
  String get notNow => '後で';

  @override
  String get record => '録音';

  @override
  String get offerToRecordCalls => '通話の録音を提案';

  @override
  String get whenTeamsZoomMeetInA =>
      'Teams、Zoom、ブラウザーの Meet、その他の通話アプリがマイクを使い始めたとき。「録音」をタップするまで何も録音されません。';

  @override
  String get stopWhenTheCallEnds => '通話が終わったら停止';

  @override
  String get onlyForRecordingsStartedFromThe =>
      '提案から開始した録音のみ。通話アプリがマイクを離して 20 秒後に停止します。';

  @override
  String get readSpeakerNamesFromTheCall => '通話ウィンドウから話者の名前を読み取る';

  @override
  String get whileRecordingKoalinoReadsWhoTeams =>
      '録音中、Koalino は Teams、Zoom、Meet が話者として表示する人物を読み取り、不明な声の名前候補にします。データはこのコンピューターの外に出ず、「割り当て」をタップするまで誰にも名前は付きません。';

  @override
  String get deleteThisChat => 'このチャットを削除しますか？';

  @override
  String get itsQuestionsAndAnswersAreDeleted => '質問と回答はこのデバイスから削除されます。';

  @override
  String get delete => '削除';

  @override
  String get thatFileIsNoLongerIn => 'そのファイルはもうプロジェクトにありません。';

  @override
  String get whoAnswers => '回答するもの';

  @override
  String get myPairedComputer => 'ペアリング済みのコンピューター';

  @override
  String get thisPhoneSAiConnection => 'このスマートフォンの AI 接続';

  @override
  String get deleteThisChat2 => 'このチャットを削除';

  @override
  String get askAboutYourNotesAnswersCome =>
      'メモについて質問できます。回答はペアリング済みのコンピューターから届きます。コンピューターは自身のメモのコピーを、他のデバイス用に選んだ接続で読み取ります。';

  @override
  String get askAboutYourNotesTheAnswer =>
      'メモについて質問できます。回答はデフォルトの AI 接続から届き、収まる範囲のメモが送られます。';

  @override
  String get yourQuestion => '質問';

  @override
  String get ask => '質問する';

  @override
  String get open => '開く';

  @override
  String given(Object included) {
    return '提供: $included';
  }

  @override
  String leftOutForLength(Object omitted) {
    return '長さのため除外: $omitted';
  }

  @override
  String answeredBy(Object connection) {
    return '回答: $connection';
  }

  @override
  String get unableToOpenTheKeychain => 'キーチェーンを開けませんでした';

  @override
  String get koalinoKeepsTheKeyThatEncrypts =>
      'Koalino は、声のプロファイルとデバイス ID を暗号化する鍵をシステムのキーチェーンに保管しています。キーチェーンのロックを解除するか、Koalino の使用を許可してから、もう一度お試しください。このデバイスにキーチェーンサービスがない場合は、インストールまたは起動して Koalino を再起動してください。';

  @override
  String get unableToOpenYourNotes => 'メモを開けませんでした';

  @override
  String get koalinoCouldNotOpenItsDatabase =>
      'Koalino がデータベースを開けませんでした。空き容量を確認して、もう一度お試しください。';

  @override
  String get unlockingYourNotes => 'メモのロックを解除中…';

  @override
  String get quit => '終了';

  @override
  String get retry => '再試行';

  @override
  String delete2(Object title) {
    return '$title を削除しますか？';
  }

  @override
  String get itsTranscriptAttachmentsAndUnknownVoices =>
      '文字起こし、添付ファイル、不明な声は、ここでもペアリング済みのデバイスでも完全に削除されます。保存した人物は残ります。';

  @override
  String get unableToDeleteThisMeetingIf =>
      'この会議を削除できませんでした。録音中の場合は、先に録音を停止してください。';

  @override
  String get devices => 'デバイス';

  @override
  String get deviceSyncIsOff => 'デバイス同期はオフです';

  @override
  String get meetingsStayOnThisDeviceUntil => '同期をオンにするまで、会議はこのデバイスにのみ保存されます。';

  @override
  String thisDevice3(Object capabilities) {
    return 'このデバイス\n$capabilities';
  }

  @override
  String get renameThisDevice => 'このデバイスの名前を変更';

  @override
  String get discoverableYourOtherDevicesCanAsk =>
      '検出可能: 数分間、他のデバイスからペアリングを申請できます。';

  @override
  String get makeDiscoverable => '検出可能にする';

  @override
  String get pairingRequests => 'ペアリングの申請';

  @override
  String get acceptOnlyIfTheOtherDevice =>
      'もう一方のデバイスに同じコードが表示されている場合のみ承認してください。';

  @override
  String get reject => '拒否';

  @override
  String get accept => '承認';

  @override
  String get pairingInProgress => 'ペアリング中';

  @override
  String get waitingForTheOtherDevice => 'もう一方のデバイスを待っています…';

  @override
  String get checkThatTheOtherDeviceShows =>
      'もう一方のデバイスに同じコードが表示されているか確認してください。';

  @override
  String get cancelPairing => 'ペアリングをキャンセル';

  @override
  String get codesMatch => 'コードが一致';

  @override
  String get pairedDevices => 'ペアリング済みのデバイス';

  @override
  String get noPairedDevicesYet => 'ペアリング済みのデバイスはまだありません。';

  @override
  String paired(Object pairedAt, Object hasRelay) {
    return '$pairedAt にペアリング\n$hasRelay';
  }

  @override
  String remove3(Object deviceName) {
    return '$deviceName を削除';
  }

  @override
  String get nearbyDevices => '近くのデバイス';

  @override
  String get pair => 'ペアリング';

  @override
  String get anotherNetwork => '別のネットワーク';

  @override
  String get showAnInviteHereAndUse =>
      'ここに招待を表示して、もう一方のデバイスで使ってください。ペアリング前にはコードの照合も行います。';

  @override
  String get showInvite => '招待を表示';

  @override
  String get useInvite => '招待を使う';

  @override
  String stopsSyncingWithThisDevicePair(Object deviceName) {
    return '$deviceName はこのデバイスとの同期を停止します。再開するには再度ペアリングしてください。';
  }

  @override
  String pairingCode(Object split) {
    return 'ペアリングコード $split';
  }

  @override
  String get noOtherDevicesFoundOnThis =>
      'このネットワークに他のデバイスは見つかりませんでした。「近く」は同じ Wi-Fi 上のデバイスのみ検出します。別のネットワークのデバイスには招待を使ってください。';

  @override
  String get noOtherDevicesFoundOnThis2 =>
      'このネットワークに他のデバイスは見つかりませんでした。「近く」は同じ Wi-Fi 上のデバイスのみ検出します。別のネットワークのデバイスには招待を使うか、設定でリレーを指定してください。';

  @override
  String get notConnectedItReconnectsWhenIt => '未接続。オンラインになりリレーに届くと再接続します。';

  @override
  String get notConnectedPutBothDevicesOn =>
      '未接続。両方のデバイスを同じ Wi-Fi に接続するか、設定でリレーを指定してください。';

  @override
  String get whisperOnDevice => 'Whisper（デバイス上）';

  @override
  String get openaiCloud => 'OpenAI（クラウド）';

  @override
  String gbGpu(Object round) {
    return '$round GB GPU';
  }

  @override
  String mbGpu(Object gpuMegabytes) {
    return '$gpuMegabytes MB GPU';
  }

  @override
  String get noTranscriptionModels => '文字起こしモデルなし';

  @override
  String get useYourOwnAiApp => '自分の AI アプリを使う';

  @override
  String get theMeetingItsNotesAndIts =>
      '会議とそのメモ、ファイルは、あなたのアカウントで選んだアプリに送られます。要約が欲しい場合は、そこで回答をコピーして戻るか、Koalino に共有してください。Koalino が保存します。';

  @override
  String summarizeIn(Object displayName) {
    return '$displayName で要約';
  }

  @override
  String chatIn(Object displayName) {
    return '$displayName でチャット';
  }

  @override
  String get summarizeInAnotherApp => '別のアプリで要約…';

  @override
  String get chatInAnotherApp => '別のアプリでチャット…';

  @override
  String summaryFromSavedTo(Object appName, Object title) {
    return '$appName の要約を「$title」に保存しました。';
  }

  @override
  String get undo => '元に戻す';

  @override
  String get theEarlierSummaryIsBack => '以前の要約に戻しました。';

  @override
  String toKeepTheSummaryCopyS(Object appName) {
    return '要約を残すには、$appName の回答をコピーして Koalino に戻るか、回答を Koalino に共有してください。';
  }

  @override
  String get meetings => '会議';

  @override
  String get projects => 'プロジェクト';

  @override
  String get voices => '声';

  @override
  String get meeting2 => '会議';

  @override
  String get chatAboutThisMeeting => 'この会議についてチャット';

  @override
  String chat(Object meeting) {
    return 'チャット: $meeting';
  }

  @override
  String get thisMeeting => 'この会議';

  @override
  String get takePhotos => '写真を撮る';

  @override
  String get attachFiles => 'ファイルを添付';

  @override
  String get export => '書き出す';

  @override
  String shareAs(Object label) {
    return '$label として共有';
  }

  @override
  String get nameSpeakers => '話者に名前を付ける';

  @override
  String get selectAMeetingToSeeIts => '会議を選ぶと文字起こしが表示されます。';

  @override
  String get renameMeeting => '会議の名前を変更';

  @override
  String get deleteMeeting => '会議を削除';

  @override
  String recordedOn(Object devices) {
    return '$devices で録音';
  }

  @override
  String get split => '分割';

  @override
  String get longPressOrRightClickA => '箇所を長押しまたは右クリックすると修正できます。';

  @override
  String get noTranscriptIsAvailable => '文字起こしはありません。';

  @override
  String get theSpeakerIsAGuess => '話者は推定です';

  @override
  String get peopleTalkedAtOnceHereWords =>
      'ここでは複数人が同時に話していました。言葉が抜けている可能性があります';

  @override
  String get overlappingSpeech => '重なった発話';

  @override
  String get someWordsWereUnclear => '一部の言葉が不明瞭でした';

  @override
  String get addedAfterTheMeeting => '会議後に追加';

  @override
  String photoOpenFullScreen(Object fileName) {
    return '写真 $fileName、全画面で開く';
  }

  @override
  String get save => '保存';

  @override
  String get preview => 'プレビュー';

  @override
  String get editNotes => 'メモを編集';

  @override
  String get writeNotes => 'メモを書く';

  @override
  String get loading => '読み込み中…';

  @override
  String get noNotesYet => 'メモはまだありません。';

  @override
  String textAt(Object startTimeMs) {
    return '$startTimeMs のテキスト';
  }

  @override
  String get removedThisTranscriptWonTBe => '削除しました。この文字起こしは会議後に再処理されません。';

  @override
  String get savedThisTranscriptWonTBe => '保存しました。この文字起こしは会議後に再処理されません。';

  @override
  String get editText => 'テキストを編集';

  @override
  String get editTheTranscript => '文字起こしを編集';

  @override
  String get leaveItEmptyToRemoveThis => '空欄にするとこの部分を削除します。';

  @override
  String get searchMeetings => '会議を検索';

  @override
  String get chatWithAllYourNotes => 'すべてのメモとチャット';

  @override
  String get chatWithAllNotes => 'すべてのメモとチャット';

  @override
  String get refresh => '更新';

  @override
  String get noMeetingsYet => '会議はまだありません';

  @override
  String get recordingsYouStartAppearHere => '開始した録音はここに表示されます。';

  @override
  String get rename => '名前を変更';

  @override
  String get addToProject => 'プロジェクトに追加…';

  @override
  String get summary2 => '要約…';

  @override
  String get addToProject2 => 'プロジェクトに追加';

  @override
  String writingTheSummaryOf(Object title) {
    return '$title の要約を作成中';
  }

  @override
  String summaryOf(Object title) {
    return '$title の要約';
  }

  @override
  String summarizeWith(Object label) {
    return '$label で要約';
  }

  @override
  String defaultLabel2(Object model) {
    return '$model · デフォルト';
  }

  @override
  String get summarizeOnMyComputer => '自分のコンピューターで要約';

  @override
  String get withClaudeCodeCodexOrThe => 'Claude Code、Codex、またはそこで設定した接続を使用';

  @override
  String get useYourOwnAiApp2 => '自分の AI アプリを使う…';

  @override
  String get claudeChatgptOrAnotherAppWith =>
      'Claude、ChatGPT、その他のアプリをあなたのアカウントで';

  @override
  String get openSummary => '要約を開く';

  @override
  String get editPromptForThisMeeting => 'この会議のプロンプトを編集…';

  @override
  String get useTheSettingsPrompt => '設定のプロンプトを使う';

  @override
  String get promptForThisMeeting => 'この会議のプロンプト';

  @override
  String placeholders(Object placeholders) {
    return 'プレースホルダー: $placeholders';
  }

  @override
  String get addAnAiConnectionInSettings3 => '要約を作成するには、設定で AI 接続を追加してください。';

  @override
  String get summarizeWith2 => '要約に使う接続';

  @override
  String get summarize => '要約';

  @override
  String get summarizeAgain => 'もう一度要約';

  @override
  String get editPromptForThisMeeting2 => 'この会議のプロンプトを編集';

  @override
  String get writingTheSummaryALocalModel => '要約を作成中… ローカルモデルでは数分かかることがあります。';

  @override
  String get noSummaryYet => '要約はまだありません。';

  @override
  String get writeASummary => '要約を書く';

  @override
  String writtenByYouOn(Object createdAt) {
    return '$createdAt にあなたが作成';
  }

  @override
  String writtenByOn(
      Object model, Object connectionLabel, Object createdAt, Object edited) {
    return '$createdAt に $model（$connectionLabel）が作成$edited';
  }

  @override
  String get editSummary => '要約を編集';

  @override
  String get tryAgainOnMyComputer => '自分のコンピューターで再試行';

  @override
  String get cancelRequest => 'リクエストをキャンセル';

  @override
  String get notesFolder => 'メモフォルダー';

  @override
  String get keepYourProjectsAndMeetingsAs =>
      'プロジェクトと会議をフォルダー内の通常のファイルとして保存し、任意のエディターで読み書きできます。notes.md、summary.md、プロジェクトのファイルへの変更は 30 秒以内にアプリへ反映され、プロジェクトのフォルダーや会議の添付ファイルフォルダーに置いたファイルは追加されます。transcript.md はアプリが書き換えるので、文字起こしの修正はアプリで行ってください。';

  @override
  String get theFolderIsADecryptedCopy =>
      'このフォルダーはメモの復号済みコピーです。フォルダーを読める人は誰でもメモを読めます。バックアップや同期のツールもコピーします。暗号化されたディスク上の自分のユーザーフォルダー内に置いてください。';

  @override
  String get folder => 'フォルダー';

  @override
  String get chooseAFolder => 'フォルダーを選ぶ';

  @override
  String get keepTheNotesFolder => 'メモフォルダーを維持';

  @override
  String get syncNow => '今すぐ同期';

  @override
  String lastSyncAtFailed(Object at, Object code) {
    return '$at の前回の同期に失敗しました: $code';
  }

  @override
  String lastSyncAtFilesWrittenEdits(
      Object at, Object written, Object imported) {
    return '前回の同期 $at: $written 件のファイルを書き込み、$imported 件の変更とファイルを取り込みました。';
  }

  @override
  String get inviteADevice => 'デバイスを招待';

  @override
  String get pairingInviteQrCode => 'ペアリング招待の QR コード';

  @override
  String onTheOtherDeviceOpenDevices(Object hour, Object minute) {
    return 'もう一方のデバイスで「デバイス」を開き、「招待を使う」を選んでこのリンクを貼り付けてください。カメラアプリでコードを読み取ってコピーすることもできます。$hour:$minute まで有効です。';
  }

  @override
  String get close => '閉じる';

  @override
  String get inviteLinkCopied => '招待リンクをコピーしました';

  @override
  String get copyLink => 'リンクをコピー';

  @override
  String get deviceFoundChoosePairUnderNearby =>
      'デバイスが見つかりました。「近くのデバイス」で「ペアリング」を選んでください。';

  @override
  String get useAnInvite => '招待を使う';

  @override
  String get inviteLink => '招待リンク';

  @override
  String get findDevice => 'デバイスを探す';

  @override
  String get pairWithAnotherDevice => '別のデバイスとペアリングしますか？';

  @override
  String get youOpenedAPairingInviteKoalino =>
      'ペアリングの招待を開きました。Koalino が招待を表示しているデバイスを探し、何かを共有する前に両方のデバイスに照合用のコードが表示されます。';

  @override
  String get pairingStartedCheckThatBothDevices =>
      'ペアリングを開始しました。両方のデバイスに同じコードが表示されているか確認してください。';

  @override
  String get done => '完了';

  @override
  String get photosAreAddedToTheMeeting => '撮影した写真はその都度会議に追加されます。';

  @override
  String get takePhoto => '写真を撮る';

  @override
  String get theCameraIsUnavailable => 'カメラを使用できません。';

  @override
  String search(Object name) {
    return '$name を検索';
  }

  @override
  String get noteSaved => 'メモを保存しました';

  @override
  String get theNoteCouldNotBeSaved => 'メモを保存できませんでした。';

  @override
  String couldNotBeAdded2(Object failed) {
    return '$failed 件は追加できませんでした。';
  }

  @override
  String get renameProject => 'プロジェクトの名前を変更';

  @override
  String get theProjectCouldNotBeRenamed => 'プロジェクトの名前を変更できませんでした。';

  @override
  String delete3(Object project) {
    return '「$project」を削除しますか？';
  }

  @override
  String get itsFilesNotesAndEMails =>
      'ファイル、メモ、メールはペアリング済みのすべてのデバイスで削除されます。会議はどのプロジェクトにも属さない状態で残ります。';

  @override
  String get theProjectCouldNotBeDeleted => 'プロジェクトを削除できませんでした。';

  @override
  String get itIsDeletedOnEveryPaired => 'ペアリング済みのすべてのデバイスで削除されます。';

  @override
  String get theFileCouldNotBeDeleted => 'ファイルを削除できませんでした。';

  @override
  String get theMeetingCouldNotBeRemoved => '会議をプロジェクトから外せませんでした。';

  @override
  String get project => 'プロジェクト';

  @override
  String get searchThisProject => 'このプロジェクトを検索';

  @override
  String get chatAboutThisProject => 'このプロジェクトについてチャット';

  @override
  String get addFiles => 'ファイルを追加';

  @override
  String get writeANote => 'メモを書く';

  @override
  String get addMeetings => '会議を追加';

  @override
  String get deleteProject => 'プロジェクトを削除';

  @override
  String get addThemHereOrFromA => 'ここで追加するか、会議のページから追加してください。';

  @override
  String get removeFromProject => 'プロジェクトから外す';

  @override
  String get filesNotesAndEMails => 'ファイル、メモ、メール';

  @override
  String get nothingAddedYet => 'まだ何も追加されていません';

  @override
  String get addPicturesMarkdownPdfsEMails =>
      '画像、Markdown、PDF、メール（.eml）、文書を追加できます。';

  @override
  String get onItsWayFromAPaired => 'ペアリング済みのデバイスから転送中';

  @override
  String get newNote => '新しいメモ';

  @override
  String get noteMarkdown => 'メモ（Markdown）';

  @override
  String get everyMeetingIsAlreadyInA => 'すべての会議がすでにプロジェクトに入っています。';

  @override
  String get add => '追加';

  @override
  String get moveToProject => 'プロジェクトに移動';

  @override
  String get noProject => 'プロジェクトなし';

  @override
  String get newProject => '新しいプロジェクト…';

  @override
  String get newProject2 => '新しいプロジェクト';

  @override
  String get create => '作成';

  @override
  String get theProjectCouldNotBeCreated => 'プロジェクトを作成できませんでした。';

  @override
  String get theMeetingCouldNotBeMoved => '会議を移動できませんでした。';

  @override
  String get removedFromItsProject => 'プロジェクトから外しました';

  @override
  String movedTo(Object project) {
    return '$project に移動しました';
  }

  @override
  String get searchProjects => 'プロジェクトを検索';

  @override
  String get noProjectsYetAProjectGathers =>
      'プロジェクトはまだありません。プロジェクトは、会議とそれに関連するファイル、メモ、メールをまとめます。';

  @override
  String get theRecordingIsStillStartingTry => '録音はまだ開始中です。もう一度お試しください。';

  @override
  String recording(Object title) {
    return '「$title」を録音中';
  }

  @override
  String get recording2 => '録音中';

  @override
  String paused(Object title) {
    return '「$title」を一時停止中';
  }

  @override
  String get paused2 => '一時停止中';

  @override
  String get finishingTranscription => '文字起こしを仕上げています…';

  @override
  String get readyToRecord => '録音の準備ができました';

  @override
  String get meetingTitle => '会議のタイトル';

  @override
  String get enterMeetingTitleOptional => '会議のタイトルを入力（任意）';

  @override
  String get leaveEmptyToNameTheMeeting => '空欄にすると開始時刻を会議名にします';

  @override
  String get startingRecording => '録音を開始しています';

  @override
  String get startRecording => '録音を開始';

  @override
  String get pause => '一時停止';

  @override
  String get resume => '再開';

  @override
  String get stop => '停止';

  @override
  String get language => '言語';

  @override
  String get aChangeAppliesToWhatIs => '変更はこの後の発言から適用されます';

  @override
  String get liveTranscript => 'リアルタイム文字起こし';

  @override
  String get theTranscriptAppearsHereAFew => '話の区切りごとに、数秒後にここに文字起こしが表示されます。';

  @override
  String get nearbyDevicesSeeThisNameWhile =>
      'このデバイスが検出可能な間、近くのデバイスにこの名前が表示されます。ペアリング済みのデバイスは既に知っている名前のままです。';

  @override
  String get meetingRenamed => '会議の名前を変更しました';

  @override
  String get thisTitleIsnTAllowed => 'このタイトルは使用できません。';

  @override
  String get couldnTSaveTheNewTitle => '新しいタイトルを保存できませんでした。もう一度お試しください。';

  @override
  String get theNoteTakingEngineIsnT => 'メモ作成エンジンは現在利用できません。';

  @override
  String get thisMeetingNoLongerExists2 => 'この会議はもう存在しません。';

  @override
  String get couldnTRenameTheMeeting => '会議の名前を変更できませんでした。';

  @override
  String get thisFileIsStillOnIts => 'このファイルはまだペアリング済みのデバイスから転送中です。';

  @override
  String get thisPictureCannotBeShown => 'この画像は表示できません。';

  @override
  String get thisKindOfDocumentCanT => 'この種類の文書はまだアプリで表示できません。';

  @override
  String get thisFileCannotBeOpened => 'このファイルは開けません。';

  @override
  String get thisFileCannotBeRead => 'このファイルは読み込めません。';

  @override
  String get thisEMailCannotBeRead => 'このメールは読み込めません。';

  @override
  String get noSubject => '（件名なし）';

  @override
  String get from => '差出人';

  @override
  String get to => '宛先';

  @override
  String get cc => 'Cc';

  @override
  String get sent => '送信日時';

  @override
  String attachments(Object attachmentNames) {
    return '添付ファイル: $attachmentNames';
  }

  @override
  String get theRestOfThisEMail => 'このメールの残りは長すぎて表示できません。';

  @override
  String get searching => '検索中…';

  @override
  String get nothingFound => '見つかりませんでした。';

  @override
  String get search2 => '検索';

  @override
  String get pairYourDevicesAndSeeWhich => 'デバイスをペアリングし、接続状況を確認します。';

  @override
  String get transcriptionModel => '文字起こしモデル';

  @override
  String get chooseTheWhisperModel => 'Whisper のモデルを選びます。';

  @override
  String get languageModelsForMeetingSummariesAnd => '会議の要約に使う言語モデルと、要約のプロンプト。';

  @override
  String get keepProjectsAndMeetingsAsFiles =>
      'プロジェクトと会議を、任意のエディターで編集できるファイルとして保存します。';

  @override
  String get letYourOwnAssistantSuchAs =>
      'Claude Code など自分のアシスタントにメモの読み取りを許可します。';

  @override
  String get appLock => 'アプリロック';

  @override
  String get defaultLanguage => 'デフォルトの言語';

  @override
  String get usedForNewRecordingsAutoDetects =>
      '新しい録音で使われます。「自動」は話している言語を検出します。';

  @override
  String get transcribeOn => '文字起こしするデバイス';

  @override
  String get anotherDeviceReceivesTheRecordingS =>
      '別のデバイスが録音の音声を暗号化された状態で受け取り、文字起こし後に削除します。そのデバイスが使えない場合は、このデバイスが文字起こしします。';

  @override
  String get telemetry => 'テレメトリー';

  @override
  String get howLongTranscriptionTakesOnEach => '各デバイスでの文字起こしの所要時間と推定消費電力。';

  @override
  String get logLevel => 'ログレベル';

  @override
  String get howMuchDetailTheAppWrites =>
      'アプリがログに書き込む詳細さ。「デバッグ」と「トレース」は問題の診断に役立ちますが、書き込み量が増えます。';

  @override
  String get custom => 'カスタム';

  @override
  String get shareLogs => 'ログを共有';

  @override
  String get theLast7DaysOfDiagnostic => '過去 7 日間の診断ログ。不具合報告に添付するときなどに。';

  @override
  String get unableToReadTheAppLock => 'アプリロックの設定を読み取れませんでした。';

  @override
  String get needsAndroid10OrLater => 'Android 10 以降が必要です。';

  @override
  String get setAScreenLockInAndroid =>
      'アプリロックを使うには、Android の設定で画面ロックを設定してください。';

  @override
  String askForYourFingerprintFaceOr(Object lockAfterBackground) {
    return '$lockAfterBackground 秒以上離れてから Koalino に戻ったときに、指紋、顔、または画面ロックを求めます。ロック中も録音と同期は続きます。';
  }

  @override
  String get otherNetworks => '他のネットワーク';

  @override
  String get aRelayOnYourOwnAlways =>
      '常時稼働している自分のデバイスでリレーを動かすと、同じ Wi-Fi にいなくてもデバイス同士がつながります。同じネットワーク内の同期はリレーなしでも動作します。';

  @override
  String get relayAddress => 'リレーのアドレス';

  @override
  String get deviceAddressesOptionalOnePerLine => 'デバイスのアドレス（任意、1 行に 1 つ）';

  @override
  String get saveAddresses => 'アドレスを保存';

  @override
  String get nothingThatWasSharedCouldBe => '共有されたものを何も読み込めませんでした。';

  @override
  String couldNotBeRead(Object message, Object failedCount) {
    return '$message。$failedCount 件は読み込めませんでした';
  }

  @override
  String saveAsTheSummaryOf(Object title) {
    return '「$title」の要約として保存';
  }

  @override
  String get addToAProjectInstead => '代わりにプロジェクトに追加';

  @override
  String get splitThisMeeting => 'この会議を分割しますか？';

  @override
  String itWasRecordedOnAndThe(Object devices, Object recordings) {
    return 'この会議は $devices で録音され、アプリが録音を結合しました。分割すると、もう一方の録音（$recordings）が独自の文字起こしとともに、ここでもペアリング済みのデバイスでも別の会議として表示されます。再び結合されることはありません。この会議の文字起こしはそのまま残ります。';
  }

  @override
  String get unableToSplitThisMeeting => 'この会議を分割できませんでした。';

  @override
  String get telemetryDashboard => 'テレメトリーダッシュボード';

  @override
  String get transcription => '文字起こし';

  @override
  String get latency => 'レイテンシー';

  @override
  String get tasks => 'タスク';

  @override
  String get power => '電力';

  @override
  String get transcriptionDurationOverTime => '文字起こし時間の推移';

  @override
  String get latencyDistribution => 'レイテンシーの分布';

  @override
  String get taskTypeBreakdown => 'タスク種別の内訳';

  @override
  String get powerConsumptionEstimate => '推定消費電力';

  @override
  String get fastestLeastAccurate => '最速、精度は最低';

  @override
  String get fastManyMistakesTheAppShips => '高速、誤りが多い。アプリに同梱';

  @override
  String get goodBalanceOnMostComputers => '多くのコンピューターでバランスが良い';

  @override
  String get accurateSlowerNeedsAbout2Gb => '高精度、やや遅い、約 2 GB のメモリが必要';

  @override
  String get olderNeedsAPowerfulComputer => '旧版、高性能なコンピューターが必要';

  @override
  String get mostAccurateNeedsAPowerfulComputer => '最高精度、高性能なコンピューターが必要';

  @override
  String get veryAccurateBestWithAGraphics => '非常に高精度。グラフィックカードがあると最適';

  @override
  String get thisModelMayBeTooLarge => 'このモデルは大きすぎる可能性があります';

  @override
  String mayNeedMoreMemoryThanThis(Object model) {
    return '$model はこのデバイスのメモリ以上を必要とする可能性があり、録音が失敗したり遅くなったりすることがあります。';
  }

  @override
  String get downloadAnyway => 'それでもダウンロード';

  @override
  String get transcriptionSettingsAreUnavailable => '文字起こしの設定は利用できません。';

  @override
  String get downloadFromQualcomm => 'Qualcomm からダウンロードしますか？';

  @override
  String get about360MbIsDownloadedFrom =>
      'Qualcomm から約 360 MB をダウンロードし、約 460 MB の容量を使います:\n\n• Qualcomm の QNN ランタイム。アプリが NPU を使えるようにします。アプリのライセンスではなく、Qualcomm の AI Stack License で提供されます。\n• Whisper Small。Qualcomm AI Hub がこのスマートフォンの NPU 向けにコンパイルしたものです。\n\n各ファイルは使用前に検証されます。録音がスマートフォンの外に出ることはありません。ダウンロードすると Qualcomm の規約に同意したことになります。';

  @override
  String get acceptAndDownload => '同意してダウンロード';

  @override
  String get downloadingTheNpuFiles => 'NPU 用ファイルをダウンロード中…';

  @override
  String get transcribeOnTheNpu => 'NPU で文字起こし';

  @override
  String get thisPhoneHasAnAiChip =>
      'このスマートフォンには Whisper Small を実行できる AI チップ（NPU）があり、プロセッサーを空けておけます。30 秒の発話を約 2 秒で処理します。Qualcomm からのダウンロードが必要です。';

  @override
  String get download => 'ダウンロード';

  @override
  String get runsWhisperSmallOnThePhone =>
      '下で選んだモデルの代わりにスマートフォンの AI チップで Whisper Small を実行します。会議後の処理に専用のモデルがない場合はそれにも使います。単語の時刻はおおよそです。';

  @override
  String get removeTheNpuFiles => 'NPU 用ファイルを削除';

  @override
  String get alsoRecordTheComputerSSound => 'コンピューターの音声も録音';

  @override
  String get transcribesWhatTheComputerPlaysSuch =>
      'マイクに加えて、通話相手の声などコンピューターが再生する音声も文字起こしします。マイクがスピーカーの音も拾った場合、重複した言葉は 1 回だけ残します。';

  @override
  String get improveTheTranscriptAfterTheMeeting => '会議後に文字起こしを改善';

  @override
  String get transcribesEachMeetingAgainWithMore =>
      '会議の終了後、より多くの文脈で再度文字起こしします。スマートフォンは充電中になるまで待ちます。それまで音声は暗号化して保管され、その後削除されます。';

  @override
  String get modelForThisPass => 'この処理に使うモデル';

  @override
  String get aLargerModelIsSlowerBut =>
      '大きいモデルほど遅いですが精度が高く、会議後なら時間があります。上でモデルをダウンロードしてください。';

  @override
  String get sameAsForRecording => '録音時と同じ';

  @override
  String get keepSpeechForCombining => '統合のために音声を保管';

  @override
  String get whenAMeetingWasAlsoRecorded =>
      '会議が別のデバイスでも録音されていた場合、その音声はこの期間だけ統合を待ち、その後削除されます。';

  @override
  String get whisperModel => 'Whisper モデル';

  @override
  String get biggerModelsMakeFewerMistakesAnd =>
      '大きいモデルほど誤りが少なくなりますが、時間とメモリが多く必要です。ダウンロード元は Hugging Face です。この選択はこのデバイスにのみ適用されます。';

  @override
  String automatic2(Object fileName, Object mode) {
    return '自動: $fileName$mode';
  }

  @override
  String usesUntilIsDownloadedTheBest(
      Object fileName, Object waiting, Object reason) {
    return 'このデバイスに最適な $waiting のダウンロードが終わるまで $fileName を使います。$reason';
  }

  @override
  String picksTheBestModelForThis(Object fileName, Object mode) {
    return 'このデバイスに最適なモデルを選びます: $fileName$mode。';
  }

  @override
  String download2(Object sizeBytes) {
    return '$sizeBytes をダウンロード';
  }

  @override
  String tryLabel(Object model) {
    return '$model を試す';
  }

  @override
  String get use => '使う';

  @override
  String suggestedForThisDevice(Object model, Object mode) {
    return 'このデバイスにおすすめ: $model$mode';
  }

  @override
  String get efficientMode => '効率モード';

  @override
  String runsTheSameModelsOnThe(Object gpuName) {
    return '同じモデルをグラフィックスチップ（$gpuName）で実行します。通常はより高速です。オフにすると通常どおりプロセッサーを使います。';
  }

  @override
  String get theGraphicsChipStoppedTheApp =>
      '前回グラフィックスチップがアプリを停止させたため、このデバイスはプロセッサーで文字起こししています。';

  @override
  String get tryTheGraphicsChipAgain => 'グラフィックスチップを再度試す';

  @override
  String get onThisDevice => 'このデバイス上';

  @override
  String get usedByAutomatic => '「自動」で使用中';

  @override
  String get englishOnly => '英語のみ';

  @override
  String get mayBeTooLargeForThis => 'このデバイスには大きすぎる可能性があります';

  @override
  String get doesnTMatchYourLanguage => '言語が一致しません';

  @override
  String get notEnoughFreeSpace => '空き容量が不足しています';

  @override
  String get cancelDownload => 'ダウンロードをキャンセル';

  @override
  String get glossary => '用語集';

  @override
  String get namesProjectNamesAndTermsOne =>
      '人名、プロジェクト名、用語を 1 行に 1 つずつ。文字起こしはここに書いたとおりに表記します。';

  @override
  String get saveGlossary => '用語集を保存';

  @override
  String get voiceGallery => '声のギャラリー';

  @override
  String get tryAgain => '再試行';

  @override
  String get unknownVoices => '不明な声';

  @override
  String get noUnknownSpeakersToReview => '確認が必要な不明な話者はいません';

  @override
  String get savedPeople => '保存した人物';

  @override
  String get noSavedPeopleYetNameA => '保存した人物はまだいません。声に名前を付けると追加されます。';

  @override
  String delete4(Object displayName) {
    return '$displayName を削除';
  }

  @override
  String get playing => '再生中…';

  @override
  String get untitledMeeting => '無題の会議';

  @override
  String get play => '再生';

  @override
  String get deleteThisVoice => 'この声を削除';

  @override
  String theCallShowedSpeakingCheckThen(Object suggestedName) {
    return '通話では $suggestedName が話していると表示されていました。確認してから「割り当て」をタップしてください。';
  }

  @override
  String get chooseASavedPerson => '保存した人物を選ぶ';

  @override
  String get assign => '割り当て';

  @override
  String get deleteTheseVoices => 'これらの声を削除';

  @override
  String get theseSoundLikeOnePersonUntick =>
      '同じ人物の声のようです。別の人物のものはチェックを外してください。';

  @override
  String itsVoiceSamplesGoForGood(Object voices) {
    return '$voices 件の音声サンプルは、ここでもペアリング済みのデバイスでも完全に削除されます。文字起こしは残ります。';
  }

  @override
  String get itsVoiceSampleGoesForGood =>
      '音声サンプルは、ここでもペアリング済みのデバイスでも完全に削除されます。文字起こしは残ります。';

  @override
  String get theirNameAndVoiceSamplesGo =>
      '名前と音声サンプルは、ここでもペアリング済みのデバイスでも完全に削除されます。文字起こしは残り、番号付きの話者として表示されます。';

  @override
  String unknownSpeaker(Object index) {
    return '不明な話者 $index';
  }

  @override
  String attachedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件のファイルを添付しました',
      one: '1 件のファイルを添付しました',
    );
    return '$_temp0';
  }

  @override
  String addedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件のファイルを追加しました',
      one: '1 件のファイルを追加しました',
    );
    return '$_temp0';
  }

  @override
  String emailAttachmentsLeft(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件のメール添付ファイルはメール内に残りました',
      one: '1 件のメール添付ファイルはメール内に残りました',
    );
    return '$_temp0';
  }

  @override
  String addedMeetings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件の会議を追加しました',
      one: '1 件の会議を追加しました',
    );
    return '$_temp0';
  }

  @override
  String otherRecordings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件の録音',
      one: '1 件の録音',
    );
    return '$_temp0';
  }

  @override
  String get severalDevices => '複数のデバイス';

  @override
  String meetingCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '会議 $count 件',
      one: '会議 1 件',
    );
    return '$_temp0';
  }

  @override
  String fileCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'ファイル $count 件',
      one: 'ファイル 1 件',
    );
    return '$_temp0';
  }

  @override
  String chunkCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 区間',
      one: '1 区間',
    );
    return '$_temp0';
  }

  @override
  String resultCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件の結果',
      one: '1 件の結果',
    );
    return '$_temp0';
  }

  @override
  String get moreNotShown => '、他にも表示されていない結果があります。言葉を追加して絞り込んでください';

  @override
  String cpuCores(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'CPU $count コア',
      one: 'CPU 1 コア',
    );
    return '$_temp0';
  }

  @override
  String dayCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 日',
      one: '1 日',
    );
    return '$_temp0';
  }

  @override
  String voiceSampleCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '音声サンプル $count 件',
      one: '音声サンプル 1 件',
      zero: '音声サンプルがまだないため、声では認識されません',
    );
    return '$_temp0';
  }

  @override
  String picturesIncluded(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 枚の画像を含めました',
      one: '1 枚の画像を含めました',
    );
    return '$_temp0';
  }

  @override
  String partCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 部分',
      one: '1 部分',
    );
    return '$_temp0';
  }

  @override
  String inMeetings(Object parts, Object meetings) {
    return '$parts（$meetings）';
  }

  @override
  String assignedVoicesTo(int count, Object name) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件の声を $name に割り当てました',
      one: '$name に割り当てました',
    );
    return '$_temp0';
  }

  @override
  String addSharedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '共有された $count 件のファイルをプロジェクトに追加',
      one: '共有されたファイルをプロジェクトに追加',
    );
    return '$_temp0';
  }

  @override
  String keepSpeechSaved(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '保存しました。音声は統合のために最大 $count 日待機します。',
      one: '保存しました。音声は統合のために最大 1 日待機します。',
    );
    return '$_temp0';
  }

  @override
  String titlesCanBeAtMost(Object max) {
    return 'タイトルは最大 $max 文字です。';
  }

  @override
  String promptUnknownPlaceholder(Object example) {
    return 'プロンプトに不明または閉じていない $example があります。';
  }

  @override
  String get live => '● ライブ';

  @override
  String get offline => '○ オフライン';

  @override
  String minutesShort(Object minutes) {
    return '$minutes 分';
  }

  @override
  String get appLanguage => 'アプリの言語';

  @override
  String get appLanguageDescription => 'Koalino のメニューとメッセージの言語。';

  @override
  String get systemLanguage => 'システムのデフォルト';

  @override
  String photosAddedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '写真を $count 枚追加しました',
      one: '写真を 1 枚追加しました',
    );
    return '$_temp0';
  }

  @override
  String moreModels(int count) {
    return 'その他のモデル（$count）';
  }

  @override
  String get moreModelsHint => '少し誤りが増える軽量版、英語専用版、このデバイスでは大きすぎる可能性のあるモデルです。';

  @override
  String get microphoneSilencedByCall =>
      '別のアプリが通話でマイクを使用しているため、Android がマイクを無音にしています。通話が終わるまで何も録音されません。';

  @override
  String get handoffInTheBrowser =>
      'Koalino はブラウザーでサイトを開き（ご自身のアカウントでサインイン済み）、プロンプトと一緒に会議を渡します。次のウィンドウで、そこで行うことを説明します。';

  @override
  String toKeepTheSummaryCopyTheWebsite(String appName) {
    return '要約を保存するには、ウェブサイトで $appName の回答をコピーして Koalino に戻ってください。';
  }

  @override
  String get useYourClaudeOrChatgptPlan => 'Claude または ChatGPT のプランを使う';

  @override
  String get summariesCanUseYourOwnPlan =>
      '要約には API キーなしで自分の Claude または ChatGPT のサブスクリプションを使えます。Claude Code または Codex をインストールして一度サインインすれば、あとは Koalino が自動で要約を書きます。';

  @override
  String get installClaudeCode => 'Claude Code をインストール';

  @override
  String get forAClaudePlanOpensTheGuide => 'Claude のプラン向け。インストールガイドを開きます。';

  @override
  String get installCodex => 'Codex をインストール';

  @override
  String get forAChatgptPlanOpensTheGuide => 'ChatGPT のプラン向け。インストールガイドを開きます。';

  @override
  String get skipUseTheWebsite => 'スキップ：ウェブサイトを使う';

  @override
  String get koalinoCopiesTheMeetingAndOpens =>
      'インストール不要：Koalino が会議をコピーして claude.ai または chatgpt.com を開くので、そこに貼り付けます。';

  @override
  String get addAConnection => '接続を追加';

  @override
  String get claudeCodeOrCodexAlreadyInstalled =>
      'Claude Code または Codex がインストール済み、またはネットワーク上のサーバー。';

  @override
  String get noConnectionNeededUseTheWebsite =>
      '接続は不要です。会議で「要約」を開き、「自分の AI アプリを使う…」を選んでください。Koalino が会議をコピーして claude.ai または chatgpt.com を開きます。';

  @override
  String get afterInstallingAddTheConnection =>
      'インストールしてサインインしたら、ここに戻って「接続を追加 → 接続を追加 → Claude Code または Codex」を選んでください。';

  @override
  String get claudeAiOrChatgptComInYourBrowser =>
      'ブラウザーで claude.ai または chatgpt.com を自分のアカウントで';

  @override
  String get newMeeting => '新しい会議';

  @override
  String get newMeetingExplanation =>
      '録音のない会議です。メモを書いたりファイルを追加したりすると、要約とチャットで使われます。';

  @override
  String get titleOptional => 'タイトル（任意）';

  @override
  String handoffStepPressEnter(String site) {
    return '$site が会議をメッセージ欄に入れた状態で開きます。Enter キーを押して送信してください。';
  }

  @override
  String handoffStepSentByItself(String site) {
    return '$site が開き、会議を自動で送信します。求められたら先にサインインしてください。';
  }

  @override
  String handoffStepPaste(String site) {
    return '$site が開き、Koalino が会議をコピーします。メッセージ欄をクリックして貼り付け（Ctrl+V）、Enter キーを押してください。';
  }

  @override
  String get handoffStepDragFiles =>
      'ファイルの入ったフォルダーも開きます。すべてをメッセージ欄にドラッグすると、ファイルのまま送られます。';

  @override
  String get handoffStepCopyAnswer => '回答が完成したら、その下のコピーボタンを押してください。';

  @override
  String get handoffStepComeBack => 'Koalino に戻ると、回答が会議の要約になります（元に戻せます）。';

  @override
  String get handoffStepAskThere => 'その後、そこで会議について質問してください。';

  @override
  String openSite(String site) {
    return '$site を開く';
  }

  @override
  String writtenBySourceOn(String source, String createdAt) {
    return '$source が $createdAt に作成';
  }

  @override
  String get noRecordingWriteNotesOrAttach =>
      '録音はありません。メモを書くかファイルを添付してください。要約とチャットで使われます。';

  @override
  String handoffStepOpensFilled(String site) {
    return '$site が会議をメッセージ欄に入れた状態で開きます。';
  }

  @override
  String handoffStepOpensCopied(String site) {
    return '$site が開き、Koalino が会議をコピーします。';
  }

  @override
  String get handoffStepSend => 'Enter キーを押して送信します。';

  @override
  String get handoffStepPasteSend =>
      'メッセージ欄をクリックし、会議を貼り付けて（Ctrl+V）Enter キーを押します。';

  @override
  String get chatHandoffIntro =>
      '自分のアカウントで、自分の AI アプリに質問できます。Koalino がメモと文字起こしを渡し、写真やファイルはファイルのまま一緒に送られます。';

  @override
  String get plainTextFile => 'プレーンテキスト (.txt)';

  @override
  String get unableToExportTheSummary => '要約を書き出せませんでした';

  @override
  String get addAVoice => '声を追加';

  @override
  String alreadySavedVoiceAddsToThem(String name) {
    return '$name は保存済みです。この声はこの人に追加されます。';
  }

  @override
  String get addToASavedPerson => '保存済みの人に追加';

  @override
  String addVoicesTo(String name) {
    return '$name に声を追加';
  }

  @override
  String tickTheVoicesThatAre(String name) {
    return 'これらの声は誰とも認識されませんでした。$name の声にチェックを入れてください。「再生」で確認できます。';
  }

  @override
  String addTo(String name) {
    return '$name に追加';
  }

  @override
  String get apiKey => 'API キー';

  @override
  String get onlyForOnlineServicesKeptIn =>
      'オンラインサービス専用。このデバイスのキーチェーンに保存され、このアドレスにのみ送信されます。';

  @override
  String get aKeyIsSavedLeaveEmpty => 'キーは保存済みです。空欄のままにすると保持されます。';

  @override
  String get removeTheSavedKey => '保存済みのキーを削除';

  @override
  String get presetOllama =>
      '自分のコンピューターやネットワークで：無料、プライベート、オフラインで動作（Gemma、Qwen、Llama…）。';

  @override
  String get presetLmStudio => 'Ollama と同様。モデルを選べるデスクトップアプリ付き。';

  @override
  String get presetGemini =>
      'Google。制限付きの無料枠あり。無料枠では送信内容が製品改善に使われることがあります。キー：aistudio.google.com。';

  @override
  String get presetDeepseek => '従量課金、非常に安価、前払いクレジット。キー：platform.deepseek.com。';

  @override
  String get presetQwen => 'Alibaba。従量課金。キー：Alibaba Cloud Model Studio。';

  @override
  String get presetKimi => 'Moonshot AI。従量課金。キー：platform.moonshot.ai。';

  @override
  String get presetMistral =>
      'Mistral（フランス）。制限の厳しい無料枠あり。キー：console.mistral.ai。';

  @override
  String get presetOpenrouter =>
      '1 つのキーで多くのモデル（Gemini、DeepSeek、Qwen…）、一部無料。テキストは OpenRouter も経由します。キー：openrouter.ai。';

  @override
  String get keepMeetingRecordings => '会議の録音を保存';

  @override
  String get keepMeetingRecordingsSubtitle =>
      '文字起こしの後も各会議の音声をこのデバイスに暗号化して保存し、再生できるようにします。既定ではオフです。';

  @override
  String get keepMeetingRecordingsQuestion => '会議の録音を保存しますか？';

  @override
  String get keepRecordingsWarning =>
      '録音には会議の参加者全員の声が含まれます。録音する相手に伝え、地域の同意に関するルールに従ってください。多くの地域では、他人を録音するには本人の許可が必要です。録音はこのデバイスに暗号化して保存され、同期されません。音声 10 分あたり約 2 MB を使います。録音は会議と一緒に削除されるか、「すべての録音を消去」でまとめて削除されます。保存されるのは音声部分のみで、無音の部分は無音として再生されます。';

  @override
  String get keepRecordings => '録音を保存';

  @override
  String recordingsKept(int count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件の録音を保存 · $size MB',
      zero: '保存された録音はありません',
    );
    return '$_temp0';
  }

  @override
  String get wipeAllRecordings => 'すべての録音を消去';

  @override
  String get wipeAllRecordingsQuestion => 'すべての録音を消去しますか？';

  @override
  String get wipeAllRecordingsExplanation =>
      'このデバイスに保存された会議の録音はすべて削除されます。文字起こしとメモは残ります。元に戻せません。';

  @override
  String recordingsWiped(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count 件の録音を消去しました。',
      zero: '消去する録音はありません。',
    );
    return '$_temp0';
  }

  @override
  String get recordingsUnavailable => '現在、録音にアクセスできません。';

  @override
  String get playRecording => '録音を再生';

  @override
  String get stopRecordingPlayback => '再生を停止';

  @override
  String get recordingCouldNotBePlayed => '録音を再生できませんでした。';
}
