// Verifies the chat page (#97) over a fake core: the kept chat is shown, a question is sent with
// its topic and the answer says what it was given, failures are worded, and the chat is deleted.

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/chat_bridge.dart';
import 'package:note_taker_ui/presentation/chat_screen.dart';

Map<String, Object?> message(String role, String text,
        {List<String> included = const <String>[]}) =>
    <String, Object?>{
      'role': role,
      'text': text,
      'created_at': '2026-11-09T12:00:00Z',
      'included': included,
      'omitted': <String>[],
      'connection': role == 'assistant' ? 'Office' : null,
    };

void main() {
  late List<Map<String, Object?>> requests;
  late List<Map<String, Object?>> answers;

  Future<String> call(String function, List<String?> arguments) async {
    expect(function, 'note_taker_chat');
    requests.add(jsonDecode(arguments.single!) as Map<String, Object?>);
    return jsonEncode(answers.removeAt(0));
  }

  setUp(() {
    requests = <Map<String, Object?>>[];
    answers = <Map<String, Object?>>[
      <String, Object?>{
        'ok': true,
        'messages': <Object?>[
          message('user', 'Who came?'),
          message('assistant', 'Anna and Bo.'),
        ],
      },
    ];
  });

  Future<void> show(WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
      home: ChatScreen(
        bridge: ChatBridge(call: call),
        topic: const ChatTopic.meeting('m-1'),
        title: 'Chat: Review',
      ),
    ));
    await tester.pumpAndSettle();
  }

  testWidgets('a question is sent and the answer says what it was given',
      (WidgetTester tester) async {
    await show(tester);
    expect(requests.single, <String, Object?>{
      'command': 'history',
      'scope': 'meeting',
      'id': 'm-1'
    });
    expect(find.text('Who came?'), findsOneWidget);
    expect(find.text('Anna and Bo.'), findsOneWidget);

    answers.add(<String, Object?>{
      'ok': true,
      'question': message('user', 'When do we ship?'),
      'answer': message('assistant', 'On Friday.',
          included: <String>['Meeting "Review"']),
    });
    await tester.enterText(
        find.byKey(chatQuestionFieldKey), 'When do we ship?');
    await tester.tap(find.byTooltip('Ask'));
    await tester.pumpAndSettle();

    expect(requests.last['question'], 'When do we ship?');
    expect(requests.last['scope'], 'meeting');
    expect(find.text('On Friday.'), findsOneWidget);
    expect(find.text('Given: Meeting "Review"'), findsOneWidget);
    expect(find.textContaining('Answered by Office'), findsWidgets);
  });

  testWidgets('a failure is worded and the question stays',
      (WidgetTester tester) async {
    await show(tester);
    answers.add(<String, Object?>{'ok': false, 'error_code': 'no_connection'});
    await tester.enterText(find.byKey(chatQuestionFieldKey), 'Anything?');
    await tester.tap(find.byTooltip('Ask'));
    await tester.pumpAndSettle();

    expect(find.textContaining('Add an AI connection'), findsOneWidget);
    expect(find.text('Anything?'), findsOneWidget, reason: 'kept in the field');
  });

  testWidgets('the chat is deleted after asking', (WidgetTester tester) async {
    await show(tester);
    await tester.tap(find.byTooltip('Delete this chat'));
    await tester.pumpAndSettle();
    answers.add(<String, Object?>{'ok': true, 'cleared': true});
    await tester.tap(find.text('Delete'));
    await tester.pumpAndSettle();

    expect(requests.last['command'], 'clear');
    expect(find.text('Who came?'), findsNothing);
  });
}
