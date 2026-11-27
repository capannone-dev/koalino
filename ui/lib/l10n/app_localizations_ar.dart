// ignore: unused_import
import 'package:intl/intl.dart' as intl;
import 'app_localizations.dart';

// ignore_for_file: type=lint

/// The translations for Arabic (`ar`).
class AppLocalizationsAr extends AppLocalizations {
  AppLocalizationsAr([String locale = 'ar']) : super(locale);

  @override
  String get unableToCheckTheAppLock =>
      'تعذّر التحقق من قفل التطبيق. حاول مرة أخرى.';

  @override
  String get unableToShowTheUnlockPrompt =>
      'تعذّر عرض طلب إلغاء القفل. حاول مرة أخرى.';

  @override
  String get setAScreenLockOnThis => 'اضبط قفل الشاشة على هذا الجهاز أولاً.';

  @override
  String get unableToChangeTheAppLock =>
      'تعذّر تغيير قفل التطبيق. حاول مرة أخرى.';

  @override
  String get aCallApp => 'تطبيق مكالمات';

  @override
  String get call => 'مكالمة';

  @override
  String call2(Object app) {
    return 'مكالمة $app';
  }

  @override
  String theCallEndedSoTheRecording(Object tiedApp) {
    return 'انتهت مكالمة $tiedApp، لذا توقف التسجيل.';
  }

  @override
  String get restartYourBrowserAndCallApps =>
      'أعد تشغيل المتصفح وتطبيقات المكالمات مرة واحدة حتى يتمكن Koalino من معرفة من يتحدث أثناء المكالمة.';

  @override
  String get unableToLoadDevices => 'تعذّر تحميل الأجهزة';

  @override
  String get thisDeviceIsNotOnA =>
      'هذا الجهاز غير متصل بشبكة بعد. اتصل ثم حاول مرة أخرى.';

  @override
  String get thatDidNotWorkTryAgain => 'لم ينجح ذلك. حاول مرة أخرى.';

  @override
  String get use1To64CharactersWithout =>
      'استخدم من 1 إلى 64 حرفًا دون فواصل أسطر.';

  @override
  String get theNameCouldNotBeSaved => 'تعذّر حفظ الاسم. حاول مرة أخرى.';

  @override
  String get thisInviteHasExpiredMakeA =>
      'انتهت صلاحية هذه الدعوة. أنشئ دعوة جديدة على الجهاز الآخر.';

  @override
  String get thisInviteWasMadeOnThis =>
      'أُنشئت هذه الدعوة على هذا الجهاز. استخدمها على الجهاز الآخر.';

  @override
  String get thatDeviceIsAlreadyPaired => 'هذا الجهاز مقترن بالفعل.';

  @override
  String get thatIsNotAValidInvite => 'هذه ليست دعوة صالحة. انسخها مرة أخرى.';

  @override
  String get deviceSyncIsNotRunning => 'مزامنة الأجهزة لا تعمل.';

  @override
  String get theOtherDeviceIsNoLonger =>
      'لم يعد الجهاز الآخر في وضع الاقتران. حاول مرة أخرى.';

  @override
  String get theOtherAppCouldNotBe => 'تعذّر فتح التطبيق الآخر.';

  @override
  String get theOtherApp => 'التطبيق الآخر';

  @override
  String app(Object displayName) {
    return 'تطبيق $displayName';
  }

  @override
  String get unableToChangeTheLogLevel => 'تعذّر تغيير مستوى السجل';

  @override
  String logsExportedTo(Object filePath) {
    return 'صُدّرت السجلات إلى $filePath';
  }

  @override
  String get unableToShareTheLogs => 'تعذّرت مشاركة السجلات';

  @override
  String get noLogsToExportYet => 'لا توجد سجلات للتصدير بعد';

  @override
  String get unableToExportTheLogs => 'تعذّر تصدير السجلات';

  @override
  String couldNotBeAttached(Object attached, Object failedPaths) {
    return '$attached؛ تعذّر إرفاق: $failedPaths';
  }

  @override
  String get theFilesCouldNotBeAttached => 'تعذّر إرفاق الملفات.';

  @override
  String get unableToShareTheExport => 'تعذّرت مشاركة الملف المُصدَّر';

  @override
  String exportedTo(Object filePath) {
    return 'صُدّر إلى $filePath';
  }

  @override
  String get exportSaved => 'حُفظ التصدير';

  @override
  String get unableToSaveTheExport => 'تعذّر حفظ التصدير';

  @override
  String get thisMeetingNoLongerExists => 'هذا الاجتماع لم يعد موجودًا';

  @override
  String get exportIsUnavailableUntilTheApp =>
      'التصدير غير متاح حتى يكتمل تشغيل التطبيق';

  @override
  String get unableToExportTheMeeting => 'تعذّر تصدير الاجتماع';

  @override
  String get unableToLoadMeetings => 'تعذّر تحميل الاجتماعات';

  @override
  String get unableToLoadThisMeeting => 'تعذّر تحميل هذا الاجتماع';

  @override
  String get enterATitle => 'أدخل عنوانًا.';

  @override
  String titlesCanBeAtMostCharacters(
      Object maxMeetingTitleCharacters, Object runes) {
    return 'يمكن أن يتكون العنوان من $maxMeetingTitleCharacters حرفًا كحد أقصى (الحالي $runes).';
  }

  @override
  String get titlesCanTContainLineBreaks =>
      'لا يمكن أن يحتوي العنوان على فواصل أسطر أو أحرف تحكم.';

  @override
  String get notPairedYetMakeTheRelay =>
      'لم يُقترن بعد. اجعل المُرحِّل قابلاً للاكتشاف، ثم اقترن به من «الأجهزة» ← «الأجهزة القريبة».';

  @override
  String get connectingToTheRelay => 'جارٍ الاتصال بالمُرحِّل…';

  @override
  String get connectedToTheRelayAskingIt =>
      'تم الاتصال بالمُرحِّل، جارٍ طلب قبول الاتصالات…';

  @override
  String get connectedYourOtherDevicesCanReach =>
      'تم الاتصال. يمكن لأجهزتك الأخرى الوصول إلى هذا الجهاز عبر المُرحِّل.';

  @override
  String get savedThisAppliesNowWithNo =>
      'تم الحفظ. يُطبَّق الآن دون إعادة تشغيل.';

  @override
  String get thatRelayAddressIsNotValid =>
      'عنوان المُرحِّل غير صالح. يبدو هكذا: /ip4/203.0.113.5/tcp/4001/p2p/12D3Koo…';

  @override
  String get aDeviceAddressIsNotValid =>
      'أحد عناوين الأجهزة غير صالح. يجب أن ينتهي كل منها بـ /p2p/ ومعرّف الجهاز.';

  @override
  String get unableToSaveTheAddresses => 'تعذّر حفظ العناوين.';

  @override
  String get thePhotoCouldNotBeSaved => 'تعذّر حفظ الصورة.';

  @override
  String get enterAName => 'أدخل اسمًا.';

  @override
  String namesCanBeAtMostCharacters(Object maxMeetingTitleCharacters) {
    return 'يمكن أن يتكون الاسم من $maxMeetingTitleCharacters حرفًا كحد أقصى.';
  }

  @override
  String get namesCanTContainLineBreaks =>
      'لا يمكن أن يحتوي الاسم على فواصل أسطر أو أحرف تحكم.';

  @override
  String get unableToLoadProjects => 'تعذّر تحميل المشاريع';

  @override
  String get unableToLoadThisProject => 'تعذّر تحميل هذا المشروع';

  @override
  String get theFilesCouldNotBeAdded => 'تعذّرت إضافة الملفات.';

  @override
  String couldNotBeAdded(Object failedPaths) {
    return 'تعذّرت إضافة: $failedPaths';
  }

  @override
  String get unableToStartRecording => 'تعذّر بدء التسجيل';

  @override
  String get unableToPauseRecording => 'تعذّر إيقاف التسجيل مؤقتًا';

  @override
  String get unableToResumeRecording => 'تعذّر استئناف التسجيل';

  @override
  String get unableToStopRecording => 'تعذّر إيقاف التسجيل';

  @override
  String get recordingSaved => 'حُفظ التسجيل';

  @override
  String get microphoneAccessIsNeededToRecord =>
      'يلزم الوصول إلى الميكروفون للتسجيل';

  @override
  String get keepTheAppOpenAndroidDid =>
      'أبقِ التطبيق مفتوحًا: لم يسمح Android بالتسجيل في الخلفية';

  @override
  String get unableToOpenTheMicrophone => 'تعذّر فتح الميكروفون';

  @override
  String saved(Object label) {
    return 'حُفظ $label.';
  }

  @override
  String removed(Object label) {
    return 'أُزيل $label.';
  }

  @override
  String isNowTheDefault(Object id) {
    return 'أصبح $id الافتراضي.';
  }

  @override
  String answered(Object id, Object reply) {
    return 'أجاب $id: $reply';
  }

  @override
  String get promptSaved => 'حُفظت التعليمات.';

  @override
  String get theBuiltInPromptIsBack => 'تمت استعادة التعليمات المدمجة.';

  @override
  String get finishedMeetingsWillBeSummarizedAutomatically =>
      'سيتم تلخيص الاجتماعات المنتهية تلقائيًا.';

  @override
  String get automaticSummariesAreOff => 'التلخيص التلقائي متوقف.';

  @override
  String get thisComputerNoLongerWritesSummaries =>
      'لم يعد هذا الحاسوب يكتب الملخصات أو يجيب في المحادثات لأجهزتك الأخرى.';

  @override
  String nowWritesTheSummariesAndAnswers(Object id) {
    return 'يكتب $id الآن الملخصات والإجابات التي تطلبها أجهزتك الأخرى.';
  }

  @override
  String get requestCancelled => 'أُلغي الطلب.';

  @override
  String get thisMeetingUsesTheSettingsPrompt =>
      'يستخدم هذا الاجتماع تعليمات الإعدادات مجددًا.';

  @override
  String get promptSavedForThisMeeting => 'حُفظت التعليمات لهذا الاجتماع.';

  @override
  String get summarySaved => 'حُفظ الملخص.';

  @override
  String get theConnection => 'الاتصال';

  @override
  String get summaryWritten => 'كُتب الملخص.';

  @override
  String theTranscriptWasReadInParts(Object parts) {
    return 'قُرئ النص المفرَّغ على أجزاء: $parts.';
  }

  @override
  String leftOut(Object sent, Object imagesSkipped) {
    return '$sent، وتم استبعاد $imagesSkipped.';
  }

  @override
  String get theModelStoppedBeforeTheEnd =>
      'توقف النموذج قبل الوصول إلى النهاية.';

  @override
  String get waitingForYourComputerItWrites =>
      'بانتظار حاسوبك: سيكتب الملخص عندما يكون قيد التشغيل ومتصلاً بهذا الجهاز ومضبوطًا للإجابة في «الإعدادات ← اتصالات الذكاء الاصطناعي».';

  @override
  String get yourComputerIsWritingTheSummary => 'حاسوبك يكتب الملخص…';

  @override
  String yourComputerCouldNotWriteThe(Object message) {
    return 'تعذّر على حاسوبك كتابة الملخص: $message';
  }

  @override
  String get unableToRefreshTelemetry => 'تعذّر تحديث القياسات';

  @override
  String get automatic => 'تلقائي';

  @override
  String get transcriptionLanguage => 'لغة التفريغ';

  @override
  String get transcriptionLanguages => 'لغات التفريغ';

  @override
  String get theLanguageCouldNotBeChanged => 'تعذّر تغيير اللغة.';

  @override
  String get thisDevice => 'هذا الجهاز';

  @override
  String get bestAvailable => 'الأفضل المتاح';

  @override
  String get aRemovedDevice => 'جهاز محذوف';

  @override
  String get theTranscriptionDeviceCouldNotBe =>
      'تعذّر تغيير جهاز التفريغ. تحقق من أنه لا يزال مقترنًا.';

  @override
  String get downloadFinishedYouCanNowChoose =>
      'اكتمل التنزيل. يمكنك الآن اختيار هذا النموذج.';

  @override
  String get downloadCancelled => 'أُلغي التنزيل.';

  @override
  String get theDownloadStoppedNoConnectionTo =>
      'توقف التنزيل: لا يوجد اتصال بالخادم. النموذج الحالي لا يزال يعمل.';

  @override
  String get theDownloadedFileFailedItsCheck =>
      'فشل الملف المُنزَّل في التحقق فحُذف. النموذج الحالي لا يزال يعمل.';

  @override
  String get theModelCouldNotBeSaved =>
      'تعذّر حفظ النموذج: تحقق من المساحة الفارغة. النموذج الحالي لا يزال يعمل.';

  @override
  String get theDownloadServerRefusedTheFile =>
      'رفض خادم التنزيل الملف. النموذج الحالي لا يزال يعمل.';

  @override
  String get savedTheNextRecordingUsesThis =>
      'تم الحفظ. سيستخدم التسجيل التالي هذا النموذج.';

  @override
  String get savedTheNextRecordingUsesThis2 =>
      'تم الحفظ. سيُطبَّق على التسجيل التالي.';

  @override
  String get savedTheNextRecordingUsesThe =>
      'تم الحفظ. سيستخدم التسجيل التالي الاختيار التلقائي.';

  @override
  String get savedMeetingsRecordedFromNowOn =>
      'تم الحفظ. سيُطبَّق على الاجتماعات المسجلة من الآن فصاعدًا.';

  @override
  String get savedTheNextRecordingUsesThese =>
      'تم الحفظ. سيستخدم التسجيل التالي هذه الأسماء والمصطلحات.';

  @override
  String get eachTermCanHaveAtMost =>
      'يمكن أن يتكون كل مصطلح من 60 حرفًا كحد أقصى، والمجموع 500.';

  @override
  String get chooseAWindowBetween1And => 'اختر مدة بين 1 و60 يومًا.';

  @override
  String get theDownloadCouldNotStartPlease =>
      'تعذّر بدء التنزيل. حاول مرة أخرى.';

  @override
  String get savedTheNextRecordingIsTranscribed =>
      'تم الحفظ. سيُفرَّغ التسجيل التالي على وحدة NPU.';

  @override
  String get savedTheNextRecordingIsTranscribed2 =>
      'تم الحفظ. سيُفرَّغ التسجيل التالي على المعالج.';

  @override
  String get theNpuFilesAreNotAll => 'لم تُنزَّل كل ملفات NPU بعد.';

  @override
  String get theNpuFilesWereRemoved => 'أُزيلت ملفات NPU.';

  @override
  String get theNpuFilesCouldNotBe => 'تعذّرت إزالة ملفات NPU. حاول مرة أخرى.';

  @override
  String get downloadedRecordingsNowTranscribeOnThe =>
      'تم التنزيل. تُفرَّغ التسجيلات الآن على وحدة NPU.';

  @override
  String get theDownloadStoppedNoConnectionTo2 =>
      'توقف التنزيل: لا يوجد اتصال بالخادم.';

  @override
  String get aDownloadedFileFailedItsCheck =>
      'فشل أحد الملفات المُنزَّلة في التحقق فحُذف.';

  @override
  String get theFilesCouldNotBeSaved =>
      'تعذّر حفظ الملفات: تحقق من المساحة الفارغة (يلزم نحو 800 ميغابايت).';

  @override
  String get theDownloadServerRefusedTheFiles => 'رفض خادم التنزيل الملفات.';

  @override
  String get theNextRecordingTriesTheGraphics =>
      'سيجرّب التسجيل التالي شريحة الرسوميات مجددًا.';

  @override
  String get thatModelWonTBeProposed => 'لن يُقترح هذا النموذج بعد الآن.';

  @override
  String get modelRemoved => 'أُزيل النموذج.';

  @override
  String get anotherModelIsDownloadingWaitFor =>
      'يجري تنزيل نموذج آخر. انتظر حتى ينتهي.';

  @override
  String get thereIsnTEnoughFreeSpace =>
      'لا توجد مساحة فارغة كافية لهذا النموذج.';

  @override
  String get thatDidnTWorkPleaseTry => 'لم ينجح ذلك. حاول مرة أخرى.';

  @override
  String get unableToLoadVoiceGallery => 'تعذّر تحميل معرض الأصوات';

  @override
  String get unableToPlayThisVoice => 'تعذّر تشغيل هذا الصوت';

  @override
  String get unableToDeleteThisVoice => 'تعذّر حذف هذا الصوت';

  @override
  String get voiceDeleted => 'حُذف الصوت';

  @override
  String get unableToDeleteThisPerson => 'تعذّر حذف هذا الشخص';

  @override
  String deleted(Object displayName) {
    return 'حُذف $displayName';
  }

  @override
  String get enterANameBeforeAssigning => 'أدخل اسمًا قبل التعيين';

  @override
  String assignedTo(Object identityName) {
    return 'عُيّن إلى $identityName';
  }

  @override
  String assignedToThereWasNotEnough(Object identityName) {
    return 'عُيّن إلى $identityName. لم يكن هناك كلام واضح كافٍ للتعرف على هذا الصوت في التسجيلات القادمة.';
  }

  @override
  String get unableToAssignTheSpeakerCluster => 'تعذّر تعيين مجموعة المتحدث';

  @override
  String get chooseAtLeastOneVoiceTo => 'اختر صوتًا واحدًا على الأقل لتسميته';

  @override
  String thereWasNotEnoughClearSpeech(Object assigned) {
    return '$assigned. لم يكن هناك كلام واضح كافٍ للتعرف على هذا الصوت في التسجيلات القادمة.';
  }

  @override
  String get theSettingCouldNotBeSaved => 'تعذّر حفظ الإعداد.';

  @override
  String get theAppIsStillStartingTry =>
      'التطبيق لا يزال قيد التشغيل. حاول مرة أخرى.';

  @override
  String get somethingWentWrongTryAgain => 'حدث خطأ ما. حاول مرة أخرى.';

  @override
  String isUsingTheMicrophone(Object app) {
    return '$app يستخدم الميكروفون';
  }

  @override
  String get recordThisCall => 'تسجيل هذه المكالمة؟';

  @override
  String theAiServerRejectedTheRequest(Object detail) {
    return 'رفض خادم الذكاء الاصطناعي الطلب.\nرد الخادم: $detail';
  }

  @override
  String claudeCodeOrCodexCouldNot(Object detail) {
    return 'تعذّر على Claude Code أو Codex الإجابة.\nالرد: $detail';
  }

  @override
  String get typeAQuestion => 'اكتب سؤالاً.';

  @override
  String get theQuestionIsTooLong => 'السؤال طويل جدًا.';

  @override
  String get addAnAiConnectionInSettings =>
      'أضف اتصال ذكاء اصطناعي في الإعدادات، أو اختر اتصالاً افتراضيًا.';

  @override
  String get thatAiConnectionNoLongerExists =>
      'اتصال الذكاء الاصطناعي هذا لم يعد موجودًا.';

  @override
  String get thereIsNothingToTalkAbout =>
      'لا يوجد ما يمكن الحديث عنه بعد: لا نص مفرَّغ ولا ملخص ولا ملاحظات ولا ملفات.';

  @override
  String get theAiConnectionSContextIs =>
      'سياق اتصال الذكاء الاصطناعي صغير جدًا. اختر اتصالاً بسياق أكبر.';

  @override
  String get thatMeetingNoLongerExists => 'هذا الاجتماع لم يعد موجودًا.';

  @override
  String get thatProjectNoLongerExists => 'هذا المشروع لم يعد موجودًا.';

  @override
  String get theAiServerCouldNotBe =>
      'تعذّر الوصول إلى خادم الذكاء الاصطناعي. تحقق من الاتصال.';

  @override
  String get theAiServerRefusedTheApi => 'رفض خادم الذكاء الاصطناعي مفتاح API.';

  @override
  String get theAiServerIsBusyTry =>
      'خادم الذكاء الاصطناعي مشغول. حاول لاحقًا.';

  @override
  String get theModelDeclinedToAnswer => 'رفض النموذج الإجابة.';

  @override
  String get claudeCodeOrCodexIsNot =>
      'Claude Code أو Codex غير مثبت على هذا الحاسوب، أو لا يستطيع التطبيق العثور عليه.';

  @override
  String get theNotesCouldNotBeRead => 'تعذّرت قراءة الملاحظات.';

  @override
  String get noPairedComputerAnsweredOnA =>
      'لم يُجب أي حاسوب مقترن. على حاسوبك افتح «الإعدادات ← الذكاء الاصطناعي»، واختر اتصالاً ضمن «ملخصات لأجهزتي الأخرى»، وأبقِه قيد التشغيل ومتصلاً.';

  @override
  String get theComputerTookTooLongTo => 'استغرق الحاسوب وقتًا طويلاً للإجابة.';

  @override
  String get theComputerDisconnectedBeforeAnsweringTry =>
      'انقطع اتصال الحاسوب قبل الإجابة. حاول مرة أخرى.';

  @override
  String get theComputerAnswersAtMost30 =>
      'يجيب الحاسوب عن 30 سؤالاً في الساعة كحد أقصى. حاول لاحقًا.';

  @override
  String get theComputerIsAnsweringOtherQuestions =>
      'الحاسوب يجيب عن أسئلة أخرى. حاول بعد قليل.';

  @override
  String get pairedDevicesAreOffOnThis =>
      'الأجهزة المقترنة متوقفة على هذا الهاتف.';

  @override
  String get theAiServerCouldNotAnswer =>
      'تعذّر على خادم الذكاء الاصطناعي الإجابة. حاول مرة أخرى.';

  @override
  String get exportMeeting => 'تصدير الاجتماع';

  @override
  String get openaiCompatibleServer => 'خادم متوافق مع OpenAI';

  @override
  String get localServerOllama => 'خادم محلي (Ollama)';

  @override
  String get claudeCodeClaudePlan => 'Claude Code (خطة Claude)';

  @override
  String get codexChatgptPlan => 'Codex (خطة ChatGPT)';

  @override
  String get checkTheAddressAndModelPlain =>
      'تحقق من العنوان والنموذج. يعمل http:// غير المشفّر على هذا الجهاز أو الشبكة المحلية فقط.';

  @override
  String get thatConnectionNoLongerExists => 'هذا الاتصال لم يعد موجودًا.';

  @override
  String get addAnAiConnectionInSettings2 =>
      'أضف اتصال ذكاء اصطناعي في الإعدادات أولاً.';

  @override
  String get thisMeetingHasNoTranscriptOr =>
      'لا يحتوي هذا الاجتماع على نص مفرَّغ أو ملاحظات بعد.';

  @override
  String get thisMeetingIsTooLongFor =>
      'هذا الاجتماع طويل جدًا لهذا الاتصال. اختر اتصالاً بسياق أكبر.';

  @override
  String get theAiServerCouldNotBe2 =>
      'تعذّر الوصول إلى خادم الذكاء الاصطناعي.';

  @override
  String get theAiServerRefusedAccessUse =>
      'رفض خادم الذكاء الاصطناعي الوصول. تحقق من مفتاح API للاتصال.';

  @override
  String get theAiServerIsBusyOr =>
      'خادم الذكاء الاصطناعي مشغول أو تجاوز الحد. حاول لاحقًا.';

  @override
  String get theAiServerRejectedTheRequest2 =>
      'رفض خادم الذكاء الاصطناعي الطلب. تحقق من اسم النموذج.';

  @override
  String get theAiServerFailedTryAgain =>
      'فشل خادم الذكاء الاصطناعي. حاول مرة أخرى.';

  @override
  String get theAiServerSAnswerCould =>
      'تعذّرت قراءة رد خادم الذكاء الاصطناعي.';

  @override
  String get claudeCodeOrCodexCouldNot2 =>
      'تعذّر على Claude Code أو Codex الإجابة. تحقق من تسجيل الدخول.';

  @override
  String get theSettingsCouldNotBeSaved => 'تعذّر حفظ الإعدادات.';

  @override
  String get theMeetingCouldNotBeRead => 'تعذّرت قراءة الاجتماع أو حفظه.';

  @override
  String get error => 'خطأ';

  @override
  String get warning => 'تحذير';

  @override
  String get infoDefault => 'معلومات (افتراضي)';

  @override
  String get debug => 'تصحيح';

  @override
  String get trace => 'تتبع';

  @override
  String get thisDevice2 => 'هذا الجهاز';

  @override
  String get anotherDevice => 'جهاز آخر';

  @override
  String notConnected(Object label) {
    return '$label (غير متصل)';
  }

  @override
  String itIsKeptEncryptedUntil(Object until) {
    return 'يُحفظ مشفّرًا حتى $until.';
  }

  @override
  String get recordedOnThisDeviceOnly => 'سُجّل على هذا الجهاز فقط.';

  @override
  String recordedOnThisDeviceOnlyA(Object until) {
    return 'سُجّل على هذا الجهاز فقط. يمكن دمج تسجيل الاجتماع نفسه من جهاز آخر حتى $until.';
  }

  @override
  String combinesTheRecordingsNothingIsLeft(Object combiner) {
    return 'يدمج $combiner التسجيلات؛ لم يبقَ شيء لإرساله من هنا.';
  }

  @override
  String waitingForAudioFrom(Object names, Object kept) {
    return 'بانتظار الصوت من $names. $kept';
  }

  @override
  String combinesTheRecordingsWaitingForAudio(Object combiner, Object names) {
    return 'يدمج $combiner التسجيلات؛ بانتظار الصوت من $names.';
  }

  @override
  String get audioFromEveryDeviceIsHere =>
      'وصل الصوت من كل الأجهزة؛ سيدمج هذا الجهاز النصوص عندما يتفرغ.';

  @override
  String thisDeviceSAudioStillHas(Object combiner, Object kept) {
    return 'لم يصل صوت هذا الجهاز إلى $combiner بعد. $kept';
  }

  @override
  String get transcriptCombinedFromEveryDevice =>
      'دُمج النص المفرَّغ من كل الأجهزة.';

  @override
  String notCombinedAudioFromDidNot(Object plainNames) {
    return 'لم يُدمج: لم يصل الصوت من $plainNames خلال ثلاثة أسابيع.';
  }

  @override
  String meeting(Object createdAt) {
    return 'اجتماع $createdAt';
  }

  @override
  String get writeSomethingFirst => 'اكتب شيئًا أولاً.';

  @override
  String get theTextIsTooLong => 'النص طويل جدًا.';

  @override
  String get thatPartOfTheTranscriptChanged =>
      'تغيّر هذا الجزء من النص المفرَّغ في الأثناء. أعد فتح الاجتماع.';

  @override
  String get theMeetingCouldNotBeSaved => 'تعذّر حفظ الاجتماع.';

  @override
  String get chooseAFullFolderPathInside =>
      'اختر مسارًا كاملاً لمجلد داخل مجلدك الرئيسي (وليس المجلد الرئيسي نفسه).';

  @override
  String get theFolderCouldNotBeWritten =>
      'تعذّرت الكتابة في المجلد. تحقق من وجوده ومن صلاحية الكتابة فيه.';

  @override
  String get theNotesCouldNotBeRead2 => 'تعذّرت قراءة الملاحظات أو حفظها.';

  @override
  String get theInviteAnswerIsIncomplete => 'الرد على الدعوة غير مكتمل.';

  @override
  String get theRenameAnswerIsIncomplete => 'الرد على إعادة التسمية غير مكتمل.';

  @override
  String get thisDeviceHasNoCamera => 'لا يحتوي هذا الجهاز على كاميرا.';

  @override
  String get theCameraIsNotOpen => 'الكاميرا غير مفتوحة.';

  @override
  String get title => 'العنوان';

  @override
  String get summary => 'الملخص';

  @override
  String get notes => 'الملاحظات';

  @override
  String get transcript => 'النص المفرَّغ';

  @override
  String get projectName => 'اسم المشروع';

  @override
  String file(Object fileName) {
    return 'الملف $fileName';
  }

  @override
  String get typeWhatToLookFor => 'اكتب ما تبحث عنه.';

  @override
  String get typeAtLeastTwoLetters => 'اكتب حرفين على الأقل.';

  @override
  String get theSearchIsTooLong => 'عبارة البحث طويلة جدًا.';

  @override
  String get exportLogs => 'تصدير السجلات';

  @override
  String about(Object appName) {
    return 'حول $appName';
  }

  @override
  String get versionLicenseAndTheSoftwareIt =>
      'الإصدار والترخيص والبرمجيات المستخدمة.';

  @override
  String remove(Object label) {
    return 'إزالة $label؟';
  }

  @override
  String get itsApiKeyIsDeletedFrom =>
      'سيُحذف مفتاح API الخاص به من هذا الجهاز. تبقى الملخصات التي كتبها.';

  @override
  String get cancel => 'إلغاء';

  @override
  String get remove2 => 'إزالة';

  @override
  String get aiConnections => 'اتصالات الذكاء الاصطناعي';

  @override
  String get addConnection => 'إضافة اتصال';

  @override
  String get summariesSendTheMeetingSTranscript =>
      'ترسل الملخصات النص المفرَّغ للاجتماع وملاحظاته وصوره إلى خادم الاتصال. الخادم على شبكتك المحلية يُبقي البيانات لديك.';

  @override
  String get noConnectionsYet => 'لا توجد اتصالات بعد';

  @override
  String get addAServerOnYourNetwork =>
      'أضف خادمًا على شبكتك (Ollama أو LM Studio أو llama.cpp)، أو Claude Code أو Codex على هذا الحاسوب.';

  @override
  String get summarizeMeetingsAutomatically => 'تلخيص الاجتماعات تلقائيًا';

  @override
  String get withTheDefaultConnectionWhenA =>
      'باستخدام الاتصال الافتراضي، عندما يكتمل النص المفرَّغ للاجتماع.';

  @override
  String get summariesForMyOtherDevices => 'ملخصات لأجهزتي الأخرى';

  @override
  String get aPairedPhoneCanAskThis =>
      'يمكن لهاتف مقترن أن يطلب من هذا الحاسوب كتابة ملخص أو الإجابة في محادثة بهذا الاتصال، باستخدام نسخة الحاسوب من ملاحظاتك. يُحتسب ذلك من استخدام خطتك: حتى 10 ملخصات و30 إجابة محادثة في الساعة.';

  @override
  String get off => 'متوقف';

  @override
  String get summaryPrompt => 'تعليمات الملخص';

  @override
  String placeholdersTheTranscriptIsAlwaysSent(Object placeholders) {
    return 'العناصر النائبة: $placeholders. يُرسَل النص المفرَّغ دائمًا بعد التعليمات.';
  }

  @override
  String get savePrompt => 'حفظ التعليمات';

  @override
  String get resetToBuiltIn => 'استعادة المدمج';

  @override
  String get defaultLabel => 'افتراضي';

  @override
  String get installed => 'مثبّت';

  @override
  String get notFoundOnThisComputer => 'غير موجود على هذا الحاسوب';

  @override
  String get unencryptedHttpLocalNetworkOnly =>
      'http غير مشفّر، الشبكة المحلية فقط';

  @override
  String get runsWithYourPlanSSign => 'يعمل بتسجيل دخول خطتك على هذا الحاسوب';

  @override
  String actionsFor(Object label) {
    return 'إجراءات $label';
  }

  @override
  String get makeDefault => 'جعله افتراضيًا';

  @override
  String get test => 'اختبار';

  @override
  String get edit => 'تعديل';

  @override
  String get enterANameAnAddressAnd => 'أدخل اسمًا وعنوانًا ونموذجًا.';

  @override
  String get theContextIsANumberOf =>
      'السياق عدد من الرموز (tokens)، مثل 8192.';

  @override
  String edit2(Object label) {
    return 'تعديل $label';
  }

  @override
  String get name => 'الاسم';

  @override
  String get protocol => 'البروتوكول';

  @override
  String get runsTheToolInstalledAndSigned =>
      'يشغّل الأداة المثبتة والمسجَّل دخولها على هذا الحاسوب، فتُكتب الملخصات ضمن خطة Claude أو ChatGPT بدلاً من رصيد API. يُرسَل الاجتماع إلى Anthropic أو OpenAI بذلك الحساب.';

  @override
  String get address => 'العنوان';

  @override
  String get httpsAlwaysHttpOnlyOnThis =>
      'https:// دائمًا؛ http:// على هذا الجهاز أو الشبكة المحلية فقط.';

  @override
  String get model => 'النموذج';

  @override
  String get leaveEmptyForTheToolS =>
      'اتركه فارغًا لاستخدام الإعداد الافتراضي للأداة.';

  @override
  String get contextTokens => 'السياق (رموز)';

  @override
  String get saveConnection => 'حفظ الاتصال';

  @override
  String get koalinoIsLocked => 'Koalino مقفل';

  @override
  String get unlockWithYourFingerprintFaceOr =>
      'ألغِ القفل ببصمة الإصبع أو الوجه أو قفل الشاشة.';

  @override
  String get unlock => 'إلغاء القفل';

  @override
  String copied(Object what) {
    return 'نُسخ $what';
  }

  @override
  String get aiAssistantAccess => 'وصول مساعد الذكاء الاصطناعي';

  @override
  String get letYourOwnAiAssistantSuch =>
      'اسمح لمساعد الذكاء الاصطناعي الخاص بك، مثل Claude Code أو Codex أو Claude Desktop، بقراءة اجتماعاتك ومشاريعك: العناوين والملخصات والملاحظات والنصوص المفرَّغة وملفات المشاريع. يمكنه القراءة فقط، لا التعديل أو الحذف.';

  @override
  String get whatTheAssistantReadsMayBe =>
      'ما يقرؤه المساعد قد يُرسَل إلى نموذجه على الإنترنت بحسابك لدى تلك الخدمة. فعّله فقط للمساعدين الذين تثق بهم في محتوى اجتماعاتك.';

  @override
  String get theAssistantServerIsNotInstalled =>
      'خادم المساعد غير مثبت مع هذا التطبيق.';

  @override
  String get letMyAiAssistantReadMy =>
      'السماح لمساعد الذكاء الاصطناعي بقراءة ملاحظاتي';

  @override
  String get connectYourAssistant => 'اربط مساعدك';

  @override
  String get yourNotesAreUnlockedWithA =>
      'تُفتح ملاحظاتك بعبارة مرور، ولا تُكتب أبدًا في إعدادات المساعد: يقرأ الخادم ملاحظاتك فقط إذا كان NOTE_TAKER_ENCRYPTION_PASSPHRASE مضبوطًا في البيئة التي يشغّله منها المساعد.';

  @override
  String get claudeCodeRunInATerminal => 'Claude Code: شغّل في الطرفية';

  @override
  String get command => 'الأمر';

  @override
  String get claudeDesktopAndOtherMcpClients =>
      'Claude Desktop وعملاء MCP الآخرون: أضف إلى إعداداتهم';

  @override
  String get settings => 'الإعدادات';

  @override
  String copy(Object title) {
    return 'نسخ $title';
  }

  @override
  String isUsingTheMicrophoneRecordThis(Object app) {
    return '$app يستخدم الميكروفون. تسجيل هذه المكالمة؟';
  }

  @override
  String get notNow => 'ليس الآن';

  @override
  String get record => 'تسجيل';

  @override
  String get offerToRecordCalls => 'اقتراح تسجيل المكالمات';

  @override
  String get whenTeamsZoomMeetInA =>
      'عندما يبدأ Teams أو Zoom أو Meet في المتصفح أو أي تطبيق مكالمات آخر باستخدام الميكروفون. لا يُسجَّل شيء حتى تنقر «تسجيل».';

  @override
  String get stopWhenTheCallEnds => 'الإيقاف عند انتهاء المكالمة';

  @override
  String get onlyForRecordingsStartedFromThe =>
      'فقط للتسجيلات التي بدأت من الاقتراح: يتوقف بعد 20 ثانية من تحرير تطبيق المكالمات للميكروفون.';

  @override
  String get readSpeakerNamesFromTheCall =>
      'قراءة أسماء المتحدثين من نافذة المكالمة';

  @override
  String get whileRecordingKoalinoReadsWhoTeams =>
      'أثناء التسجيل يقرأ Koalino من يُظهره Teams أو Zoom أو Meet متحدثًا، ويقترح ذلك الاسم للأصوات غير المعروفة. لا يغادر شيء هذا الحاسوب، ولا يحصل أحد على اسم حتى تنقر «تعيين».';

  @override
  String get deleteThisChat => 'حذف هذه المحادثة؟';

  @override
  String get itsQuestionsAndAnswersAreDeleted =>
      'ستُحذف أسئلتها وإجاباتها من هذا الجهاز.';

  @override
  String get delete => 'حذف';

  @override
  String get thatFileIsNoLongerIn => 'هذا الملف لم يعد في المشروع.';

  @override
  String get whoAnswers => 'من يجيب';

  @override
  String get myPairedComputer => 'حاسوبي المقترن';

  @override
  String get thisPhoneSAiConnection => 'اتصال الذكاء الاصطناعي لهذا الهاتف';

  @override
  String get deleteThisChat2 => 'حذف هذه المحادثة';

  @override
  String get askAboutYourNotesAnswersCome =>
      'اسأل عن ملاحظاتك. تأتي الإجابات من حاسوبك المقترن، الذي يقرأ نسخته من ملاحظاتك بالاتصال الذي اخترته لأجهزتك الأخرى.';

  @override
  String get askAboutYourNotesTheAnswer =>
      'اسأل عن ملاحظاتك. تأتي الإجابة من اتصال الذكاء الاصطناعي الافتراضي، ويُرسَل من ملاحظاتك قدر ما يتسع.';

  @override
  String get yourQuestion => 'سؤالك';

  @override
  String get ask => 'اسأل';

  @override
  String get open => 'فتح';

  @override
  String given(Object included) {
    return 'المُرسَل: $included';
  }

  @override
  String leftOutForLength(Object omitted) {
    return 'المستبعد بسبب الطول: $omitted';
  }

  @override
  String answeredBy(Object connection) {
    return 'أجاب: $connection';
  }

  @override
  String get unableToOpenTheKeychain => 'تعذّر فتح سلسلة المفاتيح';

  @override
  String get koalinoKeepsTheKeyThatEncrypts =>
      'يحتفظ Koalino بالمفتاح الذي يشفّر ملفات تعريف الأصوات وهوية الجهاز في سلسلة مفاتيح النظام. ألغِ قفلها أو اسمح لـ Koalino باستخدامها، ثم حاول مرة أخرى. إذا لم تكن لهذا الجهاز خدمة سلسلة مفاتيح، فثبّتها أو شغّلها وأعد تشغيل Koalino.';

  @override
  String get unableToOpenYourNotes => 'تعذّر فتح ملاحظاتك';

  @override
  String get koalinoCouldNotOpenItsDatabase =>
      'تعذّر على Koalino فتح قاعدة بياناته. تحقق من المساحة الفارغة وحاول مرة أخرى.';

  @override
  String get unlockingYourNotes => 'جارٍ فتح ملاحظاتك…';

  @override
  String get quit => 'خروج';

  @override
  String get retry => 'إعادة المحاولة';

  @override
  String delete2(Object title) {
    return 'حذف $title؟';
  }

  @override
  String get itsTranscriptAttachmentsAndUnknownVoices =>
      'سيُحذف نصه المفرَّغ ومرفقاته وأصواته غير المعروفة نهائيًا، هنا وعلى الأجهزة المقترنة. يبقى الأشخاص المحفوظون.';

  @override
  String get unableToDeleteThisMeetingIf =>
      'تعذّر حذف هذا الاجتماع. إذا كان قيد التسجيل، فأوقف التسجيل أولاً.';

  @override
  String get devices => 'الأجهزة';

  @override
  String get deviceSyncIsOff => 'مزامنة الأجهزة متوقفة';

  @override
  String get meetingsStayOnThisDeviceUntil =>
      'تبقى الاجتماعات على هذا الجهاز حتى تفعيل المزامنة.';

  @override
  String thisDevice3(Object capabilities) {
    return 'هذا الجهاز\n$capabilities';
  }

  @override
  String get renameThisDevice => 'إعادة تسمية هذا الجهاز';

  @override
  String get discoverableYourOtherDevicesCanAsk =>
      'قابل للاكتشاف: يمكن لأجهزتك الأخرى طلب الاقتران لبضع دقائق.';

  @override
  String get makeDiscoverable => 'جعله قابلاً للاكتشاف';

  @override
  String get pairingRequests => 'طلبات الاقتران';

  @override
  String get acceptOnlyIfTheOtherDevice =>
      'اقبل فقط إذا كان الجهاز الآخر يعرض الرمز نفسه.';

  @override
  String get reject => 'رفض';

  @override
  String get accept => 'قبول';

  @override
  String get pairingInProgress => 'الاقتران جارٍ';

  @override
  String get waitingForTheOtherDevice => 'بانتظار الجهاز الآخر…';

  @override
  String get checkThatTheOtherDeviceShows =>
      'تحقق من أن الجهاز الآخر يعرض الرمز نفسه.';

  @override
  String get cancelPairing => 'إلغاء الاقتران';

  @override
  String get codesMatch => 'الرموز متطابقة';

  @override
  String get pairedDevices => 'الأجهزة المقترنة';

  @override
  String get noPairedDevicesYet => 'لا توجد أجهزة مقترنة بعد.';

  @override
  String paired(Object pairedAt, Object hasRelay) {
    return 'اقترن في $pairedAt\n$hasRelay';
  }

  @override
  String remove3(Object deviceName) {
    return 'إزالة $deviceName';
  }

  @override
  String get nearbyDevices => 'الأجهزة القريبة';

  @override
  String get pair => 'اقتران';

  @override
  String get anotherNetwork => 'شبكة أخرى';

  @override
  String get showAnInviteHereAndUse =>
      'اعرض دعوة هنا واستخدمها على الجهاز الآخر؛ ستتحقق من الرموز قبل الاقتران.';

  @override
  String get showInvite => 'عرض الدعوة';

  @override
  String get useInvite => 'استخدام دعوة';

  @override
  String stopsSyncingWithThisDevicePair(Object deviceName) {
    return 'سيتوقف $deviceName عن المزامنة مع هذا الجهاز. اقترن مجددًا للاستئناف.';
  }

  @override
  String pairingCode(Object split) {
    return 'رمز الاقتران $split';
  }

  @override
  String get noOtherDevicesFoundOnThis =>
      'لم يُعثر على أجهزة أخرى في هذه الشبكة. «القريبة» تجد الأجهزة على شبكة Wi-Fi نفسها فقط؛ استخدم دعوة لجهاز على شبكة أخرى.';

  @override
  String get noOtherDevicesFoundOnThis2 =>
      'لم يُعثر على أجهزة أخرى في هذه الشبكة. «القريبة» تجد الأجهزة على شبكة Wi-Fi نفسها فقط؛ استخدم دعوة لجهاز على شبكة أخرى، أو حدّد مُرحِّلاً في الإعدادات.';

  @override
  String get notConnectedItReconnectsWhenIt =>
      'غير متصل؛ يُعاد الاتصال عندما يكون متصلاً بالإنترنت ويصل إلى المُرحِّل.';

  @override
  String get notConnectedPutBothDevicesOn =>
      'غير متصل. ضع الجهازين على شبكة Wi-Fi نفسها، أو حدّد مُرحِّلاً في الإعدادات.';

  @override
  String get whisperOnDevice => 'Whisper (على الجهاز)';

  @override
  String get openaiCloud => 'OpenAI (سحابي)';

  @override
  String gbGpu(Object round) {
    return '$round غيغابايت GPU';
  }

  @override
  String mbGpu(Object gpuMegabytes) {
    return '$gpuMegabytes ميغابايت GPU';
  }

  @override
  String get noTranscriptionModels => 'لا توجد نماذج تفريغ';

  @override
  String get useYourOwnAiApp => 'استخدام تطبيق الذكاء الاصطناعي الخاص بك';

  @override
  String get theMeetingItsNotesAndIts =>
      'يُرسَل الاجتماع وملاحظاته وملفاته إلى التطبيق الذي تختاره، بحسابك. إن أردت ملخصًا، انسخ الإجابة هناك وعد، أو شاركها مع Koalino: سيحفظها Koalino.';

  @override
  String summarizeIn(Object displayName) {
    return 'تلخيص في $displayName';
  }

  @override
  String chatIn(Object displayName) {
    return 'محادثة في $displayName';
  }

  @override
  String get summarizeInAnotherApp => 'تلخيص في تطبيق آخر…';

  @override
  String get chatInAnotherApp => 'محادثة في تطبيق آخر…';

  @override
  String summaryFromSavedTo(Object appName, Object title) {
    return 'حُفظ الملخص من $appName في «$title».';
  }

  @override
  String get undo => 'تراجع';

  @override
  String get theEarlierSummaryIsBack => 'تمت استعادة الملخص السابق.';

  @override
  String toKeepTheSummaryCopyS(Object appName) {
    return 'للاحتفاظ بالملخص، انسخ إجابة $appName وعد إلى Koalino، أو شارك الإجابة مع Koalino.';
  }

  @override
  String get meetings => 'الاجتماعات';

  @override
  String get projects => 'المشاريع';

  @override
  String get voices => 'الأصوات';

  @override
  String get meeting2 => 'اجتماع';

  @override
  String get chatAboutThisMeeting => 'محادثة حول هذا الاجتماع';

  @override
  String chat(Object meeting) {
    return 'محادثة: $meeting';
  }

  @override
  String get thisMeeting => 'هذا الاجتماع';

  @override
  String get takePhotos => 'التقاط صور';

  @override
  String get attachFiles => 'إرفاق ملفات';

  @override
  String get export => 'تصدير';

  @override
  String shareAs(Object label) {
    return 'مشاركة بصيغة $label';
  }

  @override
  String get nameSpeakers => 'تسمية المتحدثين';

  @override
  String get selectAMeetingToSeeIts => 'اختر اجتماعًا لعرض نصه المفرَّغ.';

  @override
  String get renameMeeting => 'إعادة تسمية الاجتماع';

  @override
  String get deleteMeeting => 'حذف الاجتماع';

  @override
  String recordedOn(Object devices) {
    return 'سُجّل على $devices';
  }

  @override
  String get split => 'فصل';

  @override
  String get longPressOrRightClickA =>
      'اضغط مطولاً أو انقر بزر الفأرة الأيمن على مقطع لتصحيحه.';

  @override
  String get noTranscriptIsAvailable => 'لا يتوفر نص مفرَّغ.';

  @override
  String get theSpeakerIsAGuess => 'المتحدث تخمين';

  @override
  String get peopleTalkedAtOnceHereWords =>
      'تحدث أكثر من شخص في الوقت نفسه هنا؛ قد تكون هناك كلمات ناقصة';

  @override
  String get overlappingSpeech => 'كلام متداخل';

  @override
  String get someWordsWereUnclear => 'بعض الكلمات لم تكن واضحة';

  @override
  String get addedAfterTheMeeting => 'أُضيف بعد الاجتماع';

  @override
  String photoOpenFullScreen(Object fileName) {
    return 'الصورة $fileName، فتح بملء الشاشة';
  }

  @override
  String get save => 'حفظ';

  @override
  String get preview => 'معاينة';

  @override
  String get editNotes => 'تعديل الملاحظات';

  @override
  String get writeNotes => 'كتابة ملاحظات';

  @override
  String get loading => 'جارٍ التحميل…';

  @override
  String get noNotesYet => 'لا توجد ملاحظات بعد.';

  @override
  String textAt(Object startTimeMs) {
    return 'النص عند $startTimeMs';
  }

  @override
  String get removedThisTranscriptWonTBe =>
      'أُزيل. لن يُعاد إنشاء هذا النص المفرَّغ بعد الاجتماع.';

  @override
  String get savedThisTranscriptWonTBe =>
      'حُفظ. لن يُعاد إنشاء هذا النص المفرَّغ بعد الاجتماع.';

  @override
  String get editText => 'تعديل النص';

  @override
  String get editTheTranscript => 'تعديل النص المفرَّغ';

  @override
  String get leaveItEmptyToRemoveThis => 'اتركه فارغًا لإزالة هذا المقطع.';

  @override
  String get searchMeetings => 'البحث في الاجتماعات';

  @override
  String get chatWithAllYourNotes => 'المحادثة مع كل ملاحظاتك';

  @override
  String get chatWithAllNotes => 'المحادثة مع كل الملاحظات';

  @override
  String get refresh => 'تحديث';

  @override
  String get noMeetingsYet => 'لا توجد اجتماعات بعد';

  @override
  String get recordingsYouStartAppearHere => 'تظهر هنا التسجيلات التي تبدؤها.';

  @override
  String get rename => 'إعادة تسمية';

  @override
  String get addToProject => 'إضافة إلى مشروع…';

  @override
  String get summary2 => 'ملخص…';

  @override
  String get addToProject2 => 'إضافة إلى مشروع';

  @override
  String writingTheSummaryOf(Object title) {
    return 'جارٍ كتابة ملخص $title';
  }

  @override
  String summaryOf(Object title) {
    return 'ملخص $title';
  }

  @override
  String summarizeWith(Object label) {
    return 'تلخيص باستخدام $label';
  }

  @override
  String defaultLabel2(Object model) {
    return '$model · افتراضي';
  }

  @override
  String get summarizeOnMyComputer => 'التلخيص على حاسوبي';

  @override
  String get withClaudeCodeCodexOrThe =>
      'باستخدام Claude Code أو Codex أو الاتصال المضبوط هناك';

  @override
  String get useYourOwnAiApp2 => 'استخدام تطبيق الذكاء الاصطناعي الخاص بك…';

  @override
  String get claudeChatgptOrAnotherAppWith =>
      'Claude أو ChatGPT أو تطبيق آخر، بحسابك';

  @override
  String get openSummary => 'فتح الملخص';

  @override
  String get editPromptForThisMeeting => 'تعديل تعليمات هذا الاجتماع…';

  @override
  String get useTheSettingsPrompt => 'استخدام تعليمات الإعدادات';

  @override
  String get promptForThisMeeting => 'تعليمات هذا الاجتماع';

  @override
  String placeholders(Object placeholders) {
    return 'العناصر النائبة: $placeholders';
  }

  @override
  String get addAnAiConnectionInSettings3 =>
      'أضف اتصال ذكاء اصطناعي في الإعدادات لكتابة الملخصات.';

  @override
  String get summarizeWith2 => 'التلخيص باستخدام';

  @override
  String get summarize => 'تلخيص';

  @override
  String get summarizeAgain => 'التلخيص مجددًا';

  @override
  String get editPromptForThisMeeting2 => 'تعديل تعليمات هذا الاجتماع';

  @override
  String get writingTheSummaryALocalModel =>
      'جارٍ كتابة الملخص… قد يستغرق النموذج المحلي بضع دقائق.';

  @override
  String get noSummaryYet => 'لا يوجد ملخص بعد.';

  @override
  String get writeASummary => 'كتابة ملخص';

  @override
  String writtenByYouOn(Object createdAt) {
    return 'كتبته أنت في $createdAt';
  }

  @override
  String writtenByOn(
      Object model, Object connectionLabel, Object createdAt, Object edited) {
    return 'كتبه $model ($connectionLabel) في $createdAt$edited';
  }

  @override
  String get editSummary => 'تعديل الملخص';

  @override
  String get tryAgainOnMyComputer => 'إعادة المحاولة على حاسوبي';

  @override
  String get cancelRequest => 'إلغاء الطلب';

  @override
  String get notesFolder => 'مجلد الملاحظات';

  @override
  String get keepYourProjectsAndMeetingsAs =>
      'احتفظ بمشاريعك واجتماعاتك كملفات عادية في مجلد، لقراءتها وتعديلها بأي محرر. تصل التعديلات على notes.md وsummary.md وملفات المشروع إلى التطبيق خلال 30 ثانية، وتُضاف الملفات الموضوعة في مجلد مشروع أو مجلد مرفقات اجتماع. يعيد التطبيق كتابة transcript.md، لذا صحّح النصوص المفرَّغة داخل التطبيق.';

  @override
  String get theFolderIsADecryptedCopy =>
      'المجلد نسخة غير مشفّرة من ملاحظاتك: يمكن لأي شخص يستطيع قراءة المجلد أن يقرأها، وأدوات النسخ الاحتياطي أو المزامنة تنسخه أيضًا. أبقِه داخل مجلد المستخدم الخاص بك على قرص مشفّر.';

  @override
  String get folder => 'المجلد';

  @override
  String get chooseAFolder => 'اختيار مجلد';

  @override
  String get keepTheNotesFolder => 'الاحتفاظ بمجلد الملاحظات';

  @override
  String get syncNow => 'مزامنة الآن';

  @override
  String lastSyncAtFailed(Object at, Object code) {
    return 'فشلت آخر مزامنة في $at: $code';
  }

  @override
  String lastSyncAtFilesWrittenEdits(
      Object at, Object written, Object imported) {
    return 'آخر مزامنة في $at: الملفات المكتوبة $written، والتعديلات والملفات المستوردة $imported.';
  }

  @override
  String get inviteADevice => 'دعوة جهاز';

  @override
  String get pairingInviteQrCode => 'رمز QR لدعوة الاقتران';

  @override
  String onTheOtherDeviceOpenDevices(Object hour, Object minute) {
    return 'على الجهاز الآخر افتح «الأجهزة»، واختر «استخدام دعوة» والصق هذا الرابط. يمكنك أيضًا مسح الرمز بتطبيق الكاميرا ونسخ النص. صالح حتى $hour:$minute.';
  }

  @override
  String get close => 'إغلاق';

  @override
  String get inviteLinkCopied => 'نُسخ رابط الدعوة';

  @override
  String get copyLink => 'نسخ الرابط';

  @override
  String get deviceFoundChoosePairUnderNearby =>
      'عُثر على الجهاز. اختر «اقتران» ضمن «الأجهزة القريبة».';

  @override
  String get useAnInvite => 'استخدام دعوة';

  @override
  String get inviteLink => 'رابط الدعوة';

  @override
  String get findDevice => 'البحث عن الجهاز';

  @override
  String get pairWithAnotherDevice => 'الاقتران بجهاز آخر؟';

  @override
  String get youOpenedAPairingInviteKoalino =>
      'فتحت دعوة اقتران. سيبحث Koalino عن الجهاز الذي يعرضها، وسيعرض الجهازان رمزًا للمطابقة قبل مشاركة أي شيء.';

  @override
  String get pairingStartedCheckThatBothDevices =>
      'بدأ الاقتران. تحقق من أن الجهازين يعرضان الرمز نفسه.';

  @override
  String get done => 'تم';

  @override
  String get photosAreAddedToTheMeeting =>
      'تُضاف الصور إلى الاجتماع فور التقاطها.';

  @override
  String get takePhoto => 'التقاط صورة';

  @override
  String get theCameraIsUnavailable => 'الكاميرا غير متاحة.';

  @override
  String search(Object name) {
    return 'البحث في $name';
  }

  @override
  String get noteSaved => 'حُفظت الملاحظة';

  @override
  String get theNoteCouldNotBeSaved => 'تعذّر حفظ الملاحظة.';

  @override
  String couldNotBeAdded2(Object failed) {
    return 'تعذّرت إضافة: $failed.';
  }

  @override
  String get renameProject => 'إعادة تسمية المشروع';

  @override
  String get theProjectCouldNotBeRenamed => 'تعذّرت إعادة تسمية المشروع.';

  @override
  String delete3(Object project) {
    return 'حذف «$project»؟';
  }

  @override
  String get itsFilesNotesAndEMails =>
      'ستُحذف ملفاته وملاحظاته ورسائله على كل الأجهزة المقترنة. تبقى اجتماعاته دون مشروع.';

  @override
  String get theProjectCouldNotBeDeleted => 'تعذّر حذف المشروع.';

  @override
  String get itIsDeletedOnEveryPaired => 'سيُحذف على كل الأجهزة المقترنة.';

  @override
  String get theFileCouldNotBeDeleted => 'تعذّر حذف الملف.';

  @override
  String get theMeetingCouldNotBeRemoved => 'تعذّرت إزالة الاجتماع من المشروع.';

  @override
  String get project => 'المشروع';

  @override
  String get searchThisProject => 'البحث في هذا المشروع';

  @override
  String get chatAboutThisProject => 'محادثة حول هذا المشروع';

  @override
  String get addFiles => 'إضافة ملفات';

  @override
  String get writeANote => 'كتابة ملاحظة';

  @override
  String get addMeetings => 'إضافة اجتماعات';

  @override
  String get deleteProject => 'حذف المشروع';

  @override
  String get addThemHereOrFromA => 'أضفها هنا أو من صفحة الاجتماع.';

  @override
  String get removeFromProject => 'إزالة من المشروع';

  @override
  String get filesNotesAndEMails => 'الملفات والملاحظات والرسائل';

  @override
  String get nothingAddedYet => 'لم يُضف شيء بعد';

  @override
  String get addPicturesMarkdownPdfsEMails =>
      'أضف صورًا وملفات Markdown وPDF ورسائل بريد (.eml) ومستندات.';

  @override
  String get onItsWayFromAPaired => 'في الطريق من جهاز مقترن';

  @override
  String get newNote => 'ملاحظة جديدة';

  @override
  String get noteMarkdown => 'ملاحظة (Markdown)';

  @override
  String get everyMeetingIsAlreadyInA =>
      'كل الاجتماعات موجودة بالفعل في مشاريع.';

  @override
  String get add => 'إضافة';

  @override
  String get moveToProject => 'نقل إلى مشروع';

  @override
  String get noProject => 'بلا مشروع';

  @override
  String get newProject => 'مشروع جديد…';

  @override
  String get newProject2 => 'مشروع جديد';

  @override
  String get create => 'إنشاء';

  @override
  String get theProjectCouldNotBeCreated => 'تعذّر إنشاء المشروع.';

  @override
  String get theMeetingCouldNotBeMoved => 'تعذّر نقل الاجتماع.';

  @override
  String get removedFromItsProject => 'أُزيل من مشروعه';

  @override
  String movedTo(Object project) {
    return 'نُقل إلى $project';
  }

  @override
  String get searchProjects => 'البحث في المشاريع';

  @override
  String get noProjectsYetAProjectGathers =>
      'لا توجد مشاريع بعد. يجمع المشروع الاجتماعات مع الملفات والملاحظات والرسائل المتعلقة بها.';

  @override
  String get theRecordingIsStillStartingTry =>
      'التسجيل لا يزال يبدأ. حاول مرة أخرى.';

  @override
  String recording(Object title) {
    return 'جارٍ تسجيل «$title»';
  }

  @override
  String get recording2 => 'جارٍ التسجيل';

  @override
  String paused(Object title) {
    return '«$title» متوقف مؤقتًا';
  }

  @override
  String get paused2 => 'متوقف مؤقتًا';

  @override
  String get finishingTranscription => 'جارٍ إنهاء التفريغ…';

  @override
  String get meetingTitle => 'عنوان الاجتماع';

  @override
  String get enterMeetingTitleOptional => 'أدخل عنوان الاجتماع (اختياري)';

  @override
  String get leaveEmptyToNameTheMeeting =>
      'اتركه فارغًا لتسمية الاجتماع بوقت بدئه';

  @override
  String get startingRecording => 'جارٍ بدء التسجيل';

  @override
  String get startRecording => 'بدء التسجيل';

  @override
  String get pause => 'إيقاف مؤقت';

  @override
  String get resume => 'استئناف';

  @override
  String get stop => 'إيقاف';

  @override
  String get language => 'اللغة';

  @override
  String get aChangeAppliesToWhatIs => 'يُطبَّق التغيير على ما يُقال بعد ذلك';

  @override
  String get liveTranscript => 'النص المفرَّغ المباشر';

  @override
  String get theTranscriptAppearsHereAFew =>
      'يظهر النص المفرَّغ هنا بعد ثوانٍ من كل توقف في الكلام.';

  @override
  String get nearbyDevicesSeeThisNameWhile =>
      'ترى الأجهزة القريبة هذا الاسم ما دام هذا الجهاز قابلاً للاكتشاف. تحتفظ الأجهزة المقترنة بالاسم الذي تعرفه.';

  @override
  String get meetingRenamed => 'أُعيدت تسمية الاجتماع';

  @override
  String get thisTitleIsnTAllowed => 'هذا العنوان غير مسموح.';

  @override
  String get couldnTSaveTheNewTitle =>
      'تعذّر حفظ العنوان الجديد. حاول مرة أخرى.';

  @override
  String get theNoteTakingEngineIsnT => 'محرك تدوين الملاحظات غير متاح حاليًا.';

  @override
  String get thisMeetingNoLongerExists2 => 'هذا الاجتماع لم يعد موجودًا.';

  @override
  String get couldnTRenameTheMeeting => 'تعذّرت إعادة تسمية الاجتماع.';

  @override
  String get thisFileIsStillOnIts =>
      'هذا الملف لا يزال في الطريق من جهاز مقترن.';

  @override
  String get thisPictureCannotBeShown => 'لا يمكن عرض هذه الصورة.';

  @override
  String get thisKindOfDocumentCanT =>
      'لا يمكن عرض هذا النوع من المستندات في التطبيق بعد.';

  @override
  String get thisFileCannotBeOpened => 'لا يمكن فتح هذا الملف.';

  @override
  String get thisFileCannotBeRead => 'لا يمكن قراءة هذا الملف.';

  @override
  String get thisEMailCannotBeRead => 'لا يمكن قراءة هذه الرسالة.';

  @override
  String get noSubject => '(بلا موضوع)';

  @override
  String get from => 'من';

  @override
  String get to => 'إلى';

  @override
  String get cc => 'نسخة';

  @override
  String get sent => 'أُرسلت';

  @override
  String attachments(Object attachmentNames) {
    return 'المرفقات: $attachmentNames';
  }

  @override
  String get theRestOfThisEMail => 'باقي هذه الرسالة أطول من أن يُعرض.';

  @override
  String get searching => 'جارٍ البحث…';

  @override
  String get nothingFound => 'لم يُعثر على شيء.';

  @override
  String get search2 => 'بحث';

  @override
  String get pairYourDevicesAndSeeWhich => 'اقرن أجهزتك واعرف أيها متصل.';

  @override
  String get transcriptionModel => 'نموذج التفريغ';

  @override
  String get chooseTheWhisperModel => 'اختر نموذج Whisper.';

  @override
  String get languageModelsForMeetingSummariesAnd =>
      'نماذج اللغة لملخصات الاجتماعات والمحادثة، وتعليماتهما.';

  @override
  String get keepProjectsAndMeetingsAsFiles =>
      'احتفظ بالمشاريع والاجتماعات كملفات يمكنك تعديلها بأي محرر.';

  @override
  String get letYourOwnAssistantSuchAs =>
      'اسمح لمساعدك الخاص، مثل Claude Code، بقراءة ملاحظاتك.';

  @override
  String get appLock => 'قفل التطبيق';

  @override
  String get defaultLanguage => 'اللغة الافتراضية';

  @override
  String get usedForNewRecordingsAutoDetects =>
      'تُستخدم للتسجيلات الجديدة. «تلقائي» يكتشف اللغة المنطوقة.';

  @override
  String get transcribeOn => 'التفريغ على';

  @override
  String get anotherDeviceReceivesTheRecordingS =>
      'يستلم جهاز آخر صوت التسجيل مشفّرًا ويحذفه بعد التفريغ. إذا لم يكن متاحًا، يفرّغ هذا الجهاز.';

  @override
  String get telemetry => 'القياسات';

  @override
  String get howLongTranscriptionTakesOnEach =>
      'مدة التفريغ على كل جهاز، والطاقة المقدّرة المستهلكة.';

  @override
  String get logLevel => 'مستوى السجل';

  @override
  String get howMuchDetailTheAppWrites =>
      'مقدار التفاصيل التي يكتبها التطبيق في سجلاته. «تصحيح» و«تتبع» يساعدان على تشخيص المشكلات لكنهما يكتبان أكثر.';

  @override
  String get custom => 'مخصص';

  @override
  String get shareLogs => 'مشاركة السجلات';

  @override
  String get theLast7DaysOfDiagnostic =>
      'سجلات التشخيص لآخر 7 أيام، مثلاً لإرفاقها بتقرير خطأ.';

  @override
  String get unableToReadTheAppLock => 'تعذّرت قراءة إعداد قفل التطبيق.';

  @override
  String get needsAndroid10OrLater => 'يتطلب Android 10 أو أحدث.';

  @override
  String get setAScreenLockInAndroid =>
      'اضبط قفل الشاشة في إعدادات Android لاستخدام قفل التطبيق.';

  @override
  String askForYourFingerprintFaceOr(Object lockAfterBackground) {
    return 'طلب بصمة الإصبع أو الوجه أو قفل الشاشة عند العودة إلى Koalino بعد غياب يزيد على $lockAfterBackground ثانية. يستمر التسجيل والمزامنة أثناء القفل.';
  }

  @override
  String get otherNetworks => 'شبكات أخرى';

  @override
  String get aRelayOnYourOwnAlways =>
      'يتيح مُرحِّل على جهاز خاص بك يعمل دائمًا وصول أجهزتك إلى بعضها حتى عندما لا تكون على شبكة Wi-Fi نفسها. المزامنة في الشبكة نفسها تعمل دونه.';

  @override
  String get relayAddress => 'عنوان المُرحِّل';

  @override
  String get deviceAddressesOptionalOnePerLine =>
      'عناوين الأجهزة (اختياري، عنوان في كل سطر)';

  @override
  String get saveAddresses => 'حفظ العناوين';

  @override
  String get nothingThatWasSharedCouldBe =>
      'تعذّرت قراءة أي شيء مما تمت مشاركته.';

  @override
  String couldNotBeRead(Object message, Object failedCount) {
    return '$message؛ تعذّرت قراءة: $failedCount';
  }

  @override
  String saveAsTheSummaryOf(Object title) {
    return 'الحفظ كملخص لـ «$title»';
  }

  @override
  String get addToAProjectInstead => 'الإضافة إلى مشروع بدلاً من ذلك';

  @override
  String get splitThisMeeting => 'فصل هذا الاجتماع؟';

  @override
  String itWasRecordedOnAndThe(Object devices, Object recordings) {
    return 'سُجّل على $devices ودمج التطبيق التسجيلات. بعد الفصل تظهر التسجيلات الأخرى ($recordings) كاجتماع مستقل بنصه المفرَّغ الخاص، هنا وعلى أجهزتك المقترنة، ولن تُدمج مجددًا. يبقى النص المفرَّغ لهذا الاجتماع كما هو.';
  }

  @override
  String get unableToSplitThisMeeting => 'تعذّر فصل هذا الاجتماع.';

  @override
  String get telemetryDashboard => 'لوحة القياسات';

  @override
  String get transcription => 'التفريغ';

  @override
  String get latency => 'زمن الاستجابة';

  @override
  String get tasks => 'المهام';

  @override
  String get power => 'الطاقة';

  @override
  String get transcriptionDurationOverTime => 'مدة التفريغ عبر الزمن';

  @override
  String get latencyDistribution => 'توزيع زمن الاستجابة';

  @override
  String get taskTypeBreakdown => 'التوزيع حسب نوع المهمة';

  @override
  String get powerConsumptionEstimate => 'تقدير استهلاك الطاقة';

  @override
  String get fastestLeastAccurate => 'الأسرع، والأقل دقة';

  @override
  String get fastManyMistakesTheAppShips =>
      'سريع، أخطاء كثيرة؛ مضمَّن في التطبيق';

  @override
  String get fastManyMistakes => 'سريع، أخطاء كثيرة';

  @override
  String get goodBalanceOnMostComputers => 'توازن جيد على معظم الحواسيب';

  @override
  String get accurateSlowerNeedsAbout2Gb =>
      'دقيق، أبطأ، يحتاج نحو 2 غيغابايت من الذاكرة';

  @override
  String get olderNeedsAPowerfulComputer => 'قديم، يحتاج حاسوبًا قويًا';

  @override
  String get mostAccurateNeedsAPowerfulComputer => 'الأدق، يحتاج حاسوبًا قويًا';

  @override
  String get veryAccurateBestWithAGraphics =>
      'دقيق جدًا، الأفضل مع بطاقة رسوميات';

  @override
  String get thisModelMayBeTooLarge => 'قد يكون هذا النموذج كبيرًا جدًا';

  @override
  String mayNeedMoreMemoryThanThis(Object model) {
    return 'قد يحتاج $model إلى ذاكرة أكثر مما لدى هذا الجهاز، فقد تفشل التسجيلات أو تبطؤ.';
  }

  @override
  String get downloadAnyway => 'التنزيل على أي حال';

  @override
  String get transcriptionSettingsAreUnavailable =>
      'إعدادات التفريغ غير متاحة.';

  @override
  String get downloadFromQualcomm => 'التنزيل من Qualcomm؟';

  @override
  String get about360MbIsDownloadedFrom =>
      'يُنزَّل نحو 360 ميغابايت من Qualcomm، ويستخدم نحو 460 ميغابايت من المساحة:\n\n• بيئة تشغيل Qualcomm QNN، التي تتيح للتطبيق استخدام NPU. تُوزَّع بموجب ترخيص Qualcomm AI Stack، لا بترخيص التطبيق.\n• Whisper Small، مُجمَّع بواسطة Qualcomm AI Hub لوحدة NPU في هذا الهاتف.\n\nيُتحقق من كل ملف قبل استخدامه. لا تغادر التسجيلات الهاتف أبدًا. بالتنزيل توافق على شروط Qualcomm.';

  @override
  String get acceptAndDownload => 'الموافقة والتنزيل';

  @override
  String get downloadingTheNpuFiles => 'جارٍ تنزيل ملفات NPU…';

  @override
  String get transcribeOnTheNpu => 'التفريغ على NPU';

  @override
  String get thisPhoneHasAnAiChip =>
      'يحتوي هذا الهاتف على شريحة ذكاء اصطناعي (NPU) يمكنها تشغيل Whisper Small مع إبقاء المعالج متاحًا: نحو ثانيتين لكل 30 ثانية من الكلام. يتطلب تنزيلاً من Qualcomm.';

  @override
  String get download => 'تنزيل';

  @override
  String get runsWhisperSmallOnThePhone =>
      'يشغّل Whisper Small على شريحة الذكاء الاصطناعي في الهاتف بدلاً من النموذج المختار أدناه، وكذلك لمعالجة ما بعد الاجتماع إن لم يكن لها نموذج خاص. توقيت الكلمات تقريبي.';

  @override
  String get removeTheNpuFiles => 'إزالة ملفات NPU';

  @override
  String get alsoRecordTheComputerSSound => 'تسجيل صوت الحاسوب أيضًا';

  @override
  String get transcribesWhatTheComputerPlaysSuch =>
      'يفرّغ ما يشغّله الحاسوب، مثل أصوات الآخرين في المكالمة، إلى جانب الميكروفون. إذا التقط الميكروفون صوت السماعات أيضًا، تُحفظ الكلمات المكررة مرة واحدة.';

  @override
  String get improveTheTranscriptAfterTheMeeting =>
      'تحسين النص المفرَّغ بعد الاجتماع';

  @override
  String get transcribesEachMeetingAgainWithMore =>
      'يفرّغ كل اجتماع مجددًا بسياق أوسع بعد انتهائه. ينتظر الهاتف حتى يكون قيد الشحن. حتى ذلك الحين يُحفظ الصوت مشفّرًا، ثم يُحذف.';

  @override
  String get modelForThisPass => 'النموذج لهذه المعالجة';

  @override
  String get aLargerModelIsSlowerBut =>
      'النموذج الأكبر أبطأ لكنه أدق، وبعد الاجتماع يتوفر الوقت. نزّل النماذج أعلاه.';

  @override
  String get sameAsForRecording => 'نفس نموذج التسجيل';

  @override
  String get keepSpeechForCombining => 'الاحتفاظ بالكلام للدمج';

  @override
  String get whenAMeetingWasAlsoRecorded =>
      'إذا سُجّل الاجتماع على جهاز آخر أيضًا، ينتظر صوته الدمج طوال هذه المدة ثم يُحذف.';

  @override
  String get whisperModel => 'نموذج Whisper';

  @override
  String get biggerModelsMakeFewerMistakesAnd =>
      'النماذج الأكبر أخطاؤها أقل لكنها تحتاج وقتًا وذاكرة أكثر. التنزيل من Hugging Face. ينطبق الاختيار على هذا الجهاز فقط.';

  @override
  String automatic2(Object fileName, Object mode) {
    return 'تلقائي: $fileName$mode';
  }

  @override
  String usesUntilIsDownloadedTheBest(
      Object fileName, Object waiting, Object reason) {
    return 'يستخدم $fileName حتى يكتمل تنزيل $waiting، الأفضل لهذا الجهاز. $reason';
  }

  @override
  String picksTheBestModelForThis(Object fileName, Object mode) {
    return 'يختار أفضل نموذج لهذا الجهاز: $fileName$mode.';
  }

  @override
  String download2(Object sizeBytes) {
    return 'تنزيل $sizeBytes';
  }

  @override
  String tryLabel(Object model) {
    return 'تجربة $model';
  }

  @override
  String get use => 'استخدام';

  @override
  String suggestedForThisDevice(Object model, Object mode) {
    return 'مقترح لهذا الجهاز: $model$mode';
  }

  @override
  String get efficientMode => 'الوضع الموفّر';

  @override
  String runsTheSameModelsOnThe(Object gpuName) {
    return 'يشغّل النماذج نفسها على شريحة الرسوميات ($gpuName)، وهو أسرع عادةً. أوقفه لاستخدام المعالج كالمعتاد.';
  }

  @override
  String get theGraphicsChipStoppedTheApp =>
      'أوقفت شريحة الرسوميات التطبيق في المرة السابقة، لذا يفرّغ هذا الجهاز على المعالج.';

  @override
  String get tryTheGraphicsChipAgain => 'تجربة شريحة الرسوميات مجددًا';

  @override
  String get onThisDevice => 'على هذا الجهاز';

  @override
  String get usedByAutomatic => 'مستخدم في الوضع التلقائي';

  @override
  String get englishOnly => 'الإنجليزية فقط';

  @override
  String get mayBeTooLargeForThis => 'قد يكون كبيرًا جدًا لهذا الجهاز';

  @override
  String get doesnTMatchYourLanguage => 'لا يطابق لغتك';

  @override
  String get notEnoughFreeSpace => 'لا توجد مساحة فارغة كافية';

  @override
  String get cancelDownload => 'إلغاء التنزيل';

  @override
  String get glossary => 'مسرد المصطلحات';

  @override
  String get namesProjectNamesAndTermsOne =>
      'الأسماء وأسماء المشاريع والمصطلحات، واحد في كل سطر. يكتبها النص المفرَّغ كما هي هنا.';

  @override
  String get saveGlossary => 'حفظ المسرد';

  @override
  String get voiceGallery => 'معرض الأصوات';

  @override
  String get tryAgain => 'إعادة المحاولة';

  @override
  String get unknownVoices => 'أصوات غير معروفة';

  @override
  String get noUnknownSpeakersToReview =>
      'لا يوجد متحدثون غير معروفين للمراجعة';

  @override
  String get savedPeople => 'الأشخاص المحفوظون';

  @override
  String get noSavedPeopleYetNameA =>
      'لا يوجد أشخاص محفوظون بعد. سمِّ صوتًا لإضافته.';

  @override
  String delete4(Object displayName) {
    return 'حذف $displayName';
  }

  @override
  String get playing => 'جارٍ التشغيل…';

  @override
  String get untitledMeeting => 'اجتماع بلا عنوان';

  @override
  String get play => 'تشغيل';

  @override
  String get deleteThisVoice => 'حذف هذا الصوت';

  @override
  String theCallShowedSpeakingCheckThen(Object suggestedName) {
    return 'أظهرت المكالمة أن $suggestedName يتحدث. تحقق ثم انقر «تعيين».';
  }

  @override
  String get chooseASavedPerson => 'اختر شخصًا محفوظًا';

  @override
  String get assign => 'تعيين';

  @override
  String get deleteTheseVoices => 'حذف هذه الأصوات';

  @override
  String get theseSoundLikeOnePersonUntick =>
      'تبدو هذه الأصوات لشخص واحد. ألغِ تحديد أي صوت لشخص آخر.';

  @override
  String itsVoiceSamplesGoForGood(Object voices) {
    return 'ستُحذف عيّنات الصوت ($voices) نهائيًا، هنا وعلى الأجهزة المقترنة. تبقى النصوص المفرَّغة.';
  }

  @override
  String get itsVoiceSampleGoesForGood =>
      'ستُحذف عيّنة الصوت نهائيًا، هنا وعلى الأجهزة المقترنة. تبقى النصوص المفرَّغة.';

  @override
  String get theirNameAndVoiceSamplesGo =>
      'سيُحذف الاسم وعيّنات الصوت نهائيًا، هنا وعلى الأجهزة المقترنة. تبقى النصوص المفرَّغة وتعرض متحدثًا مرقّمًا.';

  @override
  String unknownSpeaker(Object index) {
    return 'متحدث غير معروف $index';
  }

  @override
  String attachedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'أُرفق $count ملف',
      many: 'أُرفق $count ملفًا',
      few: 'أُرفقت $count ملفات',
      two: 'أُرفق ملفان',
      one: 'أُرفق ملف واحد',
      zero: 'لم يُرفق أي ملف',
    );
    return '$_temp0';
  }

  @override
  String addedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'أُضيف $count ملف',
      many: 'أُضيف $count ملفًا',
      few: 'أُضيفت $count ملفات',
      two: 'أُضيف ملفان',
      one: 'أُضيف ملف واحد',
      zero: 'لم يُضف أي ملف',
    );
    return '$_temp0';
  }

  @override
  String emailAttachmentsLeft(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'بقي $count مرفق داخل الرسالة',
      many: 'بقي $count مرفقًا داخل الرسالة',
      few: 'بقيت $count مرفقات داخل الرسالة',
      two: 'بقي مرفقان داخل الرسالة',
      one: 'بقي مرفق واحد داخل الرسالة',
      zero: 'لم يبقَ أي مرفق في الرسالة',
    );
    return '$_temp0';
  }

  @override
  String addedMeetings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'أُضيف $count اجتماع',
      many: 'أُضيف $count اجتماعًا',
      few: 'أُضيفت $count اجتماعات',
      two: 'أُضيف اجتماعان',
      one: 'أُضيف اجتماع واحد',
      zero: 'لم يُضف أي اجتماع',
    );
    return '$_temp0';
  }

  @override
  String otherRecordings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count تسجيل',
      many: '$count تسجيلاً',
      few: '$count تسجيلات',
      two: 'تسجيلان',
      one: 'تسجيل واحد',
      zero: 'لا تسجيلات',
    );
    return '$_temp0';
  }

  @override
  String get severalDevices => 'عدة أجهزة';

  @override
  String meetingCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count اجتماع',
      many: '$count اجتماعًا',
      few: '$count اجتماعات',
      two: 'اجتماعان',
      one: 'اجتماع واحد',
      zero: 'لا اجتماعات',
    );
    return '$_temp0';
  }

  @override
  String fileCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count ملف',
      many: '$count ملفًا',
      few: '$count ملفات',
      two: 'ملفان',
      one: 'ملف واحد',
      zero: 'لا ملفات',
    );
    return '$_temp0';
  }

  @override
  String chunkCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count مقطع',
      many: '$count مقطعًا',
      few: '$count مقاطع',
      two: 'مقطعان',
      one: 'مقطع واحد',
      zero: 'لا مقاطع',
    );
    return '$_temp0';
  }

  @override
  String resultCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count نتيجة',
      many: '$count نتيجة',
      few: '$count نتائج',
      two: 'نتيجتان',
      one: 'نتيجة واحدة',
      zero: 'لا نتائج',
    );
    return '$_temp0';
  }

  @override
  String get moreNotShown =>
      '، وهناك المزيد غير المعروض هنا؛ أضف كلمات لتضييق البحث';

  @override
  String cpuCores(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count نواة CPU',
      many: '$count نواة CPU',
      few: '$count أنوية CPU',
      two: 'نواتا CPU',
      one: 'نواة CPU واحدة',
      zero: 'لا أنوية CPU',
    );
    return '$_temp0';
  }

  @override
  String dayCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count يوم',
      many: '$count يومًا',
      few: '$count أيام',
      two: 'يومان',
      one: 'يوم واحد',
      zero: '0 يوم',
    );
    return '$_temp0';
  }

  @override
  String voiceSampleCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count عيّنة صوت',
      many: '$count عيّنة صوت',
      few: '$count عيّنات صوت',
      two: 'عيّنتا صوت',
      one: 'عيّنة صوت واحدة',
      zero: 'لا عيّنة صوت بعد، لذا لا يُتعرَّف عليه بالصوت',
    );
    return '$_temp0';
  }

  @override
  String picturesIncluded(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'ضُمّنت $count صورة',
      many: 'ضُمّنت $count صورة',
      few: 'ضُمّنت $count صور',
      two: 'ضُمّنت صورتان',
      one: 'ضُمّنت صورة واحدة',
      zero: 'لم تُضمَّن صور',
    );
    return '$_temp0';
  }

  @override
  String partCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count جزء',
      many: '$count جزءًا',
      few: '$count أجزاء',
      two: 'جزءان',
      one: 'جزء واحد',
      zero: 'لا أجزاء',
    );
    return '$_temp0';
  }

  @override
  String inMeetings(Object parts, Object meetings) {
    return '$parts في $meetings';
  }

  @override
  String assignedVoicesTo(int count, Object name) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'عُيّن $count صوت إلى $name',
      many: 'عُيّن $count صوتًا إلى $name',
      few: 'عُيّنت $count أصوات إلى $name',
      two: 'عُيّن صوتان إلى $name',
      one: 'عُيّن إلى $name',
      zero: 'لم يُعيَّن أي صوت إلى $name',
    );
    return '$_temp0';
  }

  @override
  String addSharedFiles(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'إضافة $count ملف مشترك إلى مشروع',
      many: 'إضافة $count ملفًا مشتركًا إلى مشروع',
      few: 'إضافة $count ملفات مشتركة إلى مشروع',
      two: 'إضافة الملفين المشتركين إلى مشروع',
      one: 'إضافة الملف المشترك إلى مشروع',
      zero: 'لا ملفات مشتركة لإضافتها',
    );
    return '$_temp0';
  }

  @override
  String keepSpeechSaved(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'تم الحفظ. ينتظر الكلام الدمج حتى $count يوم.',
      many: 'تم الحفظ. ينتظر الكلام الدمج حتى $count يومًا.',
      few: 'تم الحفظ. ينتظر الكلام الدمج حتى $count أيام.',
      two: 'تم الحفظ. ينتظر الكلام الدمج حتى يومين.',
      one: 'تم الحفظ. ينتظر الكلام الدمج حتى يوم واحد.',
      zero: 'تم الحفظ.',
    );
    return '$_temp0';
  }

  @override
  String titlesCanBeAtMost(Object max) {
    return 'يمكن أن يتكون العنوان من $max حرفًا كحد أقصى.';
  }

  @override
  String promptUnknownPlaceholder(Object example) {
    return 'تحتوي التعليمات على عنصر نائب غير معروف أو غير مغلق $example.';
  }

  @override
  String get live => '● مباشر';

  @override
  String get offline => '○ غير متصل';

  @override
  String minutesShort(Object minutes) {
    return '$minutes د';
  }

  @override
  String get appLanguage => 'لغة التطبيق';

  @override
  String get appLanguageDescription => 'لغة قوائم Koalino ورسائله.';

  @override
  String get systemLanguage => 'لغة النظام';

  @override
  String photosAddedCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'أُضيفت $count صورة',
      many: 'أُضيفت $count صورة',
      few: 'أُضيفت $count صور',
      two: 'أُضيفت صورتان',
      one: 'أُضيفت صورة واحدة',
      zero: 'لم تُضف صور',
    );
    return '$_temp0';
  }

  @override
  String moreModels(int count) {
    return 'نماذج أخرى ($count)';
  }

  @override
  String get moreModelsHint =>
      'إصدارات أخف تخطئ أكثر قليلاً، وإصدارات للإنجليزية فقط، ونماذج قد لا يتسع لها هذا الجهاز.';

  @override
  String get microphoneSilencedByCall =>
      'يكتم Android الميكروفون بينما يستخدمه تطبيق آخر في مكالمة: لن يُسجَّل أي شيء حتى تنتهي المكالمة.';

  @override
  String get handoffInTheBrowser =>
      'يفتح Koalino الموقع في متصفحك، حيث سجّلت الدخول بحسابك، ويعطيه الاجتماع مع موجّهك. تعرض النافذة التالية ما عليك فعله هناك.';

  @override
  String toKeepTheSummaryCopyTheWebsite(String appName) {
    return 'للاحتفاظ بالملخص، انسخ إجابة $appName من الموقع ثم عُد إلى Koalino.';
  }

  @override
  String get useYourClaudeOrChatgptPlan =>
      'استخدم اشتراكك في Claude أو ChatGPT';

  @override
  String get summariesCanUseYourOwnPlan =>
      'يمكن للملخصات استخدام اشتراكك الخاص في Claude أو ChatGPT دون مفتاح API. ثبّت Claude Code أو Codex وسجّل الدخول مرة واحدة: بعدها يكتب Koalino الملخصات بنفسه.';

  @override
  String get installClaudeCode => 'تثبيت Claude Code';

  @override
  String get forAClaudePlanOpensTheGuide =>
      'لاشتراك Claude. يفتح دليل التثبيت.';

  @override
  String get installCodex => 'تثبيت Codex';

  @override
  String get forAChatgptPlanOpensTheGuide =>
      'لاشتراك ChatGPT. يفتح دليل التثبيت.';

  @override
  String get skipUseTheWebsite => 'تخطٍّ: استخدم الموقع';

  @override
  String get koalinoCopiesTheMeetingAndOpens =>
      'لا شيء للتثبيت: ينسخ Koalino الاجتماع ويفتح claude.ai أو chatgpt.com، وتلصقه أنت هناك.';

  @override
  String get addAConnection => 'إضافة اتصال';

  @override
  String get claudeCodeOrCodexAlreadyInstalled =>
      'Claude Code أو Codex مثبّت مسبقًا، أو خادم على شبكتك.';

  @override
  String get noConnectionNeededUseTheWebsite =>
      'لا حاجة إلى اتصال. في أي اجتماع، افتح الملخص واختر «استخدام تطبيق الذكاء الاصطناعي الخاص بك…»: ينسخ Koalino الاجتماع ويفتح claude.ai أو chatgpt.com.';

  @override
  String get afterInstallingAddTheConnection =>
      'بعد التثبيت وتسجيل الدخول، عُد واختر إضافة اتصال ← إضافة اتصال ← Claude Code أو Codex.';

  @override
  String get claudeAiOrChatgptComInYourBrowser =>
      'claude.ai أو chatgpt.com في متصفحك، بحسابك';

  @override
  String get newMeeting => 'اجتماع جديد';

  @override
  String get newMeetingExplanation =>
      'اجتماع بلا تسجيل. اكتب ملاحظات أو أضف ملفات: تستخدمها الملخصات والدردشة.';

  @override
  String get titleOptional => 'العنوان (اختياري)';

  @override
  String handoffStepPressEnter(String site) {
    return 'يفتح $site والاجتماع موجود بالفعل في مربع الرسالة. اضغط Enter لإرساله.';
  }

  @override
  String handoffStepSentByItself(String site) {
    return 'يفتح $site ويرسل الاجتماع تلقائيًا. سجّل الدخول أولًا إذا طُلب منك.';
  }

  @override
  String handoffStepPaste(String site) {
    return 'يفتح $site وينسخ Koalino الاجتماع. انقر مربع الرسالة والصقه (Ctrl+V) ثم اضغط Enter.';
  }

  @override
  String get handoffStepDragFiles =>
      'يُفتح أيضًا مجلد يضم الملفات: اسحبها كلها إلى مربع الرسالة، لتُرسل كملفات.';

  @override
  String get handoffStepCopyAnswer =>
      'عند اكتمال الإجابة، اضغط زر النسخ أسفلها.';

  @override
  String get handoffStepComeBack =>
      'عُد إلى Koalino: تصبح الإجابة ملخص الاجتماع، مع إمكانية التراجع.';

  @override
  String get handoffStepAskThere => 'ثم اطرح أسئلتك عن الاجتماع هناك.';

  @override
  String openSite(String site) {
    return 'افتح $site';
  }

  @override
  String writtenBySourceOn(String source, String createdAt) {
    return 'كتبه $source في $createdAt';
  }

  @override
  String get noRecordingWriteNotesOrAttach =>
      'لا يوجد تسجيل. اكتب ملاحظات أو أرفق ملفات: تستخدمها الملخصات والدردشة.';

  @override
  String handoffStepOpensFilled(String site) {
    return 'يفتح $site والاجتماع موجود بالفعل في مربع الرسالة.';
  }

  @override
  String handoffStepOpensCopied(String site) {
    return 'يفتح $site وينسخ Koalino الاجتماع.';
  }

  @override
  String get handoffStepSend => 'اضغط Enter لإرساله.';

  @override
  String get handoffStepPasteSend =>
      'انقر مربع الرسالة والصق الاجتماع (Ctrl+V) ثم اضغط Enter.';

  @override
  String get chatHandoffIntro =>
      'اطرح أسئلتك في تطبيق الذكاء الاصطناعي الخاص بك، بحسابك الخاص. يعطيه Koalino الملاحظات والنصوص المفرّغة، وتُرسل الصور والملفات معها كملفات.';

  @override
  String get plainTextFile => 'نص عادي (.txt)';

  @override
  String get unableToExportTheSummary => 'تعذّر تصدير الملخص';

  @override
  String get addAVoice => 'إضافة صوت';

  @override
  String alreadySavedVoiceAddsToThem(String name) {
    return '$name محفوظ بالفعل: سيُضاف هذا الصوت إليه.';
  }

  @override
  String addVoicesTo(String name) {
    return 'إضافة أصوات إلى $name';
  }

  @override
  String tickTheVoicesThatAre(String name) {
    return 'لم يُتعرَّف على هذه الأصوات على أنها لأي شخص. حدّد أصوات $name؛ استخدم تشغيل للتحقق.';
  }

  @override
  String addTo(String name) {
    return 'إضافة إلى $name';
  }

  @override
  String get apiKey => 'مفتاح API';

  @override
  String get onlyForOnlineServicesKeptIn =>
      'للخدمات عبر الإنترنت فقط. يُحفظ في سلسلة مفاتيح هذا الجهاز ويُرسل إلى هذا العنوان فقط.';

  @override
  String get aKeyIsSavedLeaveEmpty =>
      'يوجد مفتاح محفوظ. اتركه فارغًا للإبقاء عليه.';

  @override
  String get removeTheSavedKey => 'إزالة المفتاح المحفوظ';

  @override
  String get presetOllama =>
      'على حاسوبك أو شبكتك: مجاني وخاص ويعمل دون اتصال (Gemma وQwen وLlama…).';

  @override
  String get presetLmStudio => 'مثل Ollama، مع تطبيق سطح مكتب لاختيار النماذج.';

  @override
  String get presetGemini =>
      'Google. فئة مجانية بحدود؛ وفيها قد تستخدم Google ما ترسله لتحسين منتجاتها. المفتاح: aistudio.google.com.';

  @override
  String get presetDeepseek =>
      'الدفع حسب الاستخدام، رخيص جدًا، برصيد مسبق. المفتاح: platform.deepseek.com.';

  @override
  String get presetQwen =>
      'Alibaba. الدفع حسب الاستخدام. المفتاح: Alibaba Cloud Model Studio.';

  @override
  String get presetKimi =>
      'Moonshot AI. الدفع حسب الاستخدام. المفتاح: platform.moonshot.ai.';

  @override
  String get presetMistral =>
      'Mistral (فرنسا). فئة مجانية بحدود ضيقة. المفتاح: console.mistral.ai.';

  @override
  String get presetOpenrouter =>
      'مفتاح واحد لنماذج كثيرة (Gemini وDeepSeek وQwen…)، بعضها مجاني؛ ويمر النص أيضًا عبر OpenRouter. المفتاح: openrouter.ai.';

  @override
  String get keepMeetingRecordings => 'الاحتفاظ بتسجيلات الاجتماعات';

  @override
  String get keepMeetingRecordingsSubtitle =>
      'احتفظ بصوت كل اجتماع بعد تفريغه نصيًا، مشفّرًا على هذا الجهاز، لتتمكن من تشغيله. معطّل افتراضيًا.';

  @override
  String get keepMeetingRecordingsQuestion =>
      'هل تريد الاحتفاظ بتسجيلات الاجتماعات؟';

  @override
  String get keepRecordingsWarning =>
      'تحتوي التسجيلات على أصوات جميع الحاضرين. أخبر الأشخاص الذين تسجّلهم، والتزم بقواعد الموافقة في بلدك: في أماكن كثيرة يتطلب تسجيل الآخرين إذنهم. تبقى التسجيلات مشفّرة على هذا الجهاز ولا تُزامَن، وتشغل نحو 2 ميغابايت لكل 10 دقائق من الكلام. تُحذف مع اجتماعها، أو كلها معًا عبر «مسح كل التسجيلات». يُحتفظ بالكلام فقط، لذا تُشغَّل فترات الصمت صمتًا.';

  @override
  String get keepRecordings => 'الاحتفاظ بالتسجيلات';

  @override
  String recordingsKept(int count, String size) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count تسجيلات محفوظة · $size ميغابايت',
      one: 'تسجيل واحد محفوظ · $size ميغابايت',
      zero: 'لا توجد تسجيلات محفوظة',
    );
    return '$_temp0';
  }

  @override
  String get wipeAllRecordings => 'مسح كل التسجيلات';

  @override
  String get wipeAllRecordingsQuestion => 'هل تريد مسح كل التسجيلات؟';

  @override
  String get wipeAllRecordingsExplanation =>
      'ستُحذف كل تسجيلات الاجتماعات المحفوظة على هذا الجهاز. تبقى النصوص والملاحظات. لا يمكن التراجع عن ذلك.';

  @override
  String recordingsWiped(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'تم مسح $count تسجيلات.',
      one: 'تم مسح تسجيل واحد.',
      zero: 'لا توجد تسجيلات لمسحها.',
    );
    return '$_temp0';
  }

  @override
  String get recordingsUnavailable => 'لا يمكن الوصول إلى التسجيلات الآن.';

  @override
  String get playRecording => 'تشغيل التسجيل';

  @override
  String get stopRecordingPlayback => 'إيقاف التشغيل';

  @override
  String get recordingCouldNotBePlayed => 'تعذّر تشغيل التسجيل.';

  @override
  String get columnWidth => 'عرض العمود';

  @override
  String get subprojects => 'المشاريع الفرعية';

  @override
  String get newSubproject => 'مشروع فرعي جديد';

  @override
  String get moveProjectTo => 'نقل إلى…';

  @override
  String get topLevel => 'المستوى الأعلى';

  @override
  String moveProjectTitle(String project) {
    return 'نقل $project إلى';
  }

  @override
  String get movedToTopLevel => 'نُقل إلى المستوى الأعلى';

  @override
  String get theProjectCouldNotBeMoved => 'تعذّر نقل المشروع.';

  @override
  String subprojectCount(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count مشروع فرعي',
      many: '$count مشروعًا فرعيًا',
      few: '$count مشاريع فرعية',
      two: 'مشروعان فرعيان',
      one: 'مشروع فرعي واحد',
      zero: 'لا مشاريع فرعية',
    );
    return '$_temp0';
  }

  @override
  String get chooseAProject => 'اختر مشروعًا لعرض اجتماعاته وملفاته.';

  @override
  String get chooseAMeetingOrFile => 'اختر اجتماعًا أو ملفًا لعرضه هنا.';

  @override
  String get backgroundPassTitle => 'تحسين النصوص حتى عند إغلاق Koalino';

  @override
  String get backgroundPassAllowed =>
      'مسموح: يحسّن الهاتف النصوص المنتظرة أثناء الشحن، حتى عند إغلاق Koalino أو تفعيل توفير الطاقة.';

  @override
  String get backgroundPassAsk =>
      'انقر للسماح لنظام Android بتنفيذ ذلك أثناء شحن الهاتف، حتى مع تفعيل توفير الطاقة. وإلا فلن يحدث إلا وKoalino مفتوح.';

  @override
  String get chatPrompt => 'موجّه الدردشة';

  @override
  String chatPromptHelp(String placeholder) {
    return 'يمثّل $placeholder ما تسأل عنه: اجتماعًا أو مشروعًا أو كل ملاحظاتك. تذكر الإجابات مصادرها دائمًا برقم، وتأتي ملاحظاتك بعد الموجّه.';
  }

  @override
  String get saveChatPrompt => 'حفظ موجّه الدردشة';

  @override
  String get chatPromptSaved => 'تم حفظ موجّه الدردشة.';

  @override
  String get theBuiltInChatPromptIsBack => 'عاد موجّه الدردشة المدمج.';

  @override
  String get promptsSharedWithPairedDevices =>
      'الموجّهان متطابقان على كل أجهزتك المقترنة: يصل أي تغيير هنا إليها عند اتصالها.';

  @override
  String get deviceOnline => 'متصل';

  @override
  String get deviceOffline => 'غير متصل';

  @override
  String get neverSynced => 'لم تتم المزامنة بعد';

  @override
  String get syncedJustNow => 'تمت المزامنة الآن';

  @override
  String lastSyncedMinutesAgo(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'آخر مزامنة قبل $count دقيقة',
      one: 'آخر مزامنة قبل دقيقة واحدة',
    );
    return '$_temp0';
  }

  @override
  String lastSyncedHoursAgo(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: 'آخر مزامنة قبل $count ساعة',
      one: 'آخر مزامنة قبل ساعة واحدة',
    );
    return '$_temp0';
  }

  @override
  String lastSyncedOn(String date) {
    return 'آخر مزامنة: $date';
  }

  @override
  String devicesOnlineCount(int online, int total) {
    return '$online من $total متصل';
  }

  @override
  String get microphoneSeemsMutedOrOff =>
      'يبدو أن الميكروفون مكتوم أو مطفأ: لا يُسمع أي شيء. تحقّق من مفتاح أو زر كتم الصوت، ومن أن الوصول إلى الميكروفون مفعّل في إعدادات النظام.';

  @override
  String get windowsBlocksTheMicrophone =>
      'يمنع Windows التطبيقات من استخدام الميكروفون: لا يُسمع أي شيء. في الإعدادات ← الخصوصية والأمان ← الميكروفون، فعّل الوصول إلى الميكروفون والسماح لتطبيقات سطح المكتب بالوصول إلى الميكروفون.';

  @override
  String get backUpAllData => 'نسخ احتياطي لكل البيانات';

  @override
  String get backUpAllDataDescription =>
      'احفظ الاجتماعات والملاحظات والمرفقات والتسجيلات المحفوظة في ملف واحد مشفّر';

  @override
  String get restoreFromBackup => 'الاستعادة من نسخة احتياطية';

  @override
  String get restoreFromBackupDescription =>
      'استبدل البيانات على هذا الجهاز بنسخة احتياطية';

  @override
  String get backupPassphraseTitle => 'عبارة مرور النسخة الاحتياطية';

  @override
  String get backupPassphraseExplanation =>
      'تُشفَّر النسخة الاحتياطية بهذه العبارة. بدونها لا يمكن لأحد فتحها، ولا حتى أنت، لذا احفظها في مكان آمن.';

  @override
  String get backupPassphraseLabel => 'عبارة المرور';

  @override
  String get repeatPassphraseLabel => 'أعد كتابة عبارة المرور';

  @override
  String get passphraseTooShort => 'استخدم 8 أحرف على الأقل.';

  @override
  String get passphrasesDoNotMatch => 'عبارتا المرور غير متطابقتين.';

  @override
  String get continueAction => 'متابعة';

  @override
  String get backingUp => 'جارٍ النسخ الاحتياطي…';

  @override
  String get restoringBackup => 'جارٍ الاستعادة…';

  @override
  String backupSaved(int files, String size) {
    return 'تم حفظ النسخة الاحتياطية: $files ملفات، $size ميغابايت.';
  }

  @override
  String get restoreConfirmTitle => 'استبدال البيانات على هذا الجهاز؟';

  @override
  String get restoreConfirmBody =>
      'تُستبدل الاجتماعات والملاحظات على هذا الجهاز بتلك الموجودة في النسخة الاحتياطية. تُحفظ البيانات الحالية جانبًا حتى الاستعادة التالية. ثم يُعاد تشغيل Koalino.';

  @override
  String get restoreAction => 'استعادة';

  @override
  String restoreReady(int files) {
    return 'تمت استعادة $files ملفات. سيُعاد تشغيل Koalino الآن لفتحها.';
  }

  @override
  String get restartNow => 'إعادة التشغيل';

  @override
  String get backupWrongPassphrase => 'عبارة المرور غير صحيحة.';

  @override
  String get backupNotABackup => 'هذا الملف ليس نسخة احتياطية من Koalino.';

  @override
  String get backupTooNew =>
      'أُنشئت هذه النسخة بإصدار أحدث من Koalino. حدّث التطبيق أولًا.';

  @override
  String get backupDamaged => 'النسخة الاحتياطية تالفة أو غير مكتملة.';

  @override
  String get backupFailed =>
      'تعذّر إكمال العملية. تحقّق من المساحة المتاحة وأعد المحاولة.';

  @override
  String get automaticBackups => 'النسخ الاحتياطي التلقائي';

  @override
  String get automaticBackupsDescription =>
      'كل يوم أثناء الشحن، في مجلد تختاره، بما في ذلك السحابة (Google Drive وNextcloud وOneDrive…). تُشفَّر قبل مغادرة الهاتف.';

  @override
  String automaticBackupFolder(String folder) {
    return 'المجلد: $folder';
  }

  @override
  String get noAutomaticBackupYet => 'لا توجد نسخة تلقائية بعد.';

  @override
  String lastAutomaticBackup(String time) {
    return 'آخر نسخة: $time';
  }

  @override
  String lastAutomaticBackupFailed(String time) {
    return 'فشلت نسخة $time. تحقّق من المجلد.';
  }

  @override
  String get backUpNow => 'انسخ الآن';

  @override
  String get backupFolderUnavailable =>
      'تعذّرت الكتابة في مجلد النسخ. تحقّق من أنه ما زال موجودًا وأن تطبيقه مسجَّل الدخول، أو اختر مجلدًا آخر.';

  @override
  String get automaticBackupsNotSetUp =>
      'يجب إعداد النسخ التلقائي من جديد: أوقفه ثم شغّله.';

  @override
  String get renameSpeaker => 'إعادة تسمية المتحدث';

  @override
  String get nameThisVoice => 'تسمية هذا الصوت';

  @override
  String get doubleClickOrLongPressToRename =>
      'انقر نقرًا مزدوجًا أو اضغط مطولًا لإعادة التسمية';

  @override
  String get theNewNameAppliesToEveryPassage =>
      'ينطبق الاسم الجديد على كل مقطع لهذا الصوت، في جميع اجتماعاتك وعلى أجهزتك المقترنة.';

  @override
  String anotherSavedPersonIsCalled(Object name) {
    return 'يوجد شخص محفوظ آخر باسم \"$name\". إذا كانا الشخص نفسه، فادمجهما.';
  }

  @override
  String get useOneTo100CharactersOnOneLine =>
      'استخدم من 1 إلى 100 حرف في سطر واحد.';

  @override
  String versionNumber(Object version) {
    return 'الإصدار $version';
  }

  @override
  String get developerSettings => 'إعدادات المطوّر';

  @override
  String get appVariablesForExpertsAndDevelopers =>
      'متغيرات التطبيق، للخبراء والمطوّرين';

  @override
  String tapsLeftToTurnOnDeveloperSettings(int count) {
    String _temp0 = intl.Intl.pluralLogic(
      count,
      locale: localeName,
      other: '$count نقرة أخرى لتشغيل إعدادات المطوّر',
      few: '$count نقرات أخرى لتشغيل إعدادات المطوّر',
      two: 'نقرتان أخريان لتشغيل إعدادات المطوّر',
      one: 'نقرة واحدة أخرى لتشغيل إعدادات المطوّر',
    );
    return '$_temp0';
  }

  @override
  String get developerSettingsAreOn => 'تم تشغيل إعدادات المطوّر';

  @override
  String get developerSettingsWarning =>
      'تغيّر هذه المتغيرات طريقة عمل Koalino الداخلية. قد توقف القيمة الخاطئة النسخ أو المزامنة أو إضافة المتصفح. تُطبَّق القيم المحفوظة عند إعادة تشغيل Koalino.';

  @override
  String get restartKoalinoToApply =>
      'أعد تشغيل Koalino لتطبيق القيم المحفوظة.';

  @override
  String nowAndDefault(Object now, Object defaultValue) {
    return 'الآن: $now · الافتراضي: $defaultValue';
  }

  @override
  String get notSet => 'غير مضبوط';

  @override
  String savedForTheNextStart(Object value) {
    return 'محفوظ للتشغيل التالي: $value';
  }

  @override
  String get useDefault => 'استخدام الافتراضي';

  @override
  String get resetAll => 'إعادة ضبط الكل';

  @override
  String get turnOffDeveloperSettings => 'إيقاف إعدادات المطوّر';

  @override
  String get turnOffDeveloperSettingsExplanation =>
      'تعود كل قيمة إلى الافتراضي عند التشغيل التالي، ويُخفى هذا القسم مجددًا.';

  @override
  String get turnOff => 'إيقاف';

  @override
  String mergeIntoAnotherPerson(Object name) {
    return 'دمج $name في شخص آخر';
  }

  @override
  String mergeNameInto(Object name) {
    return 'دمج $name في…';
  }

  @override
  String mergeWarning(Object source, Object target) {
    return 'تنتقل كل مقاطع $source وعيّنات صوته إلى $target، ويُحذف $source، على أجهزتك المقترنة أيضًا. لا يمكن التراجع عن ذلك.';
  }

  @override
  String get merge => 'دمج';

  @override
  String mergeWithName(Object name) {
    return 'دمج مع $name';
  }

  @override
  String mergedInto(Object source, Object target) {
    return 'أصبح $source جزءًا من $target';
  }

  @override
  String get thesePeopleCouldNotBeMerged =>
      'تعذّر دمج هؤلاء الأشخاص. حاول مرة أخرى.';

  @override
  String get peopleCannotBeMergedWhileRecording =>
      'لا يمكن دمج الأشخاص أثناء تسجيل اجتماع.';
}
