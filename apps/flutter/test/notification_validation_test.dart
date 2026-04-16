import 'package:infomatrix_shell/core/models.dart';
import 'package:infomatrix_shell/ui/reader_shell_page.dart';
import 'package:flutter_test/flutter_test.dart';

void main() {
  test('rejects invalid positive integer input', () {
    expect(
      validatePositiveIntInput('abc', fieldLabel: '后台刷新间隔'),
      isNotNull,
    );
    expect(
      validatePositiveIntInput('0', fieldLabel: '后台刷新间隔'),
      isNotNull,
    );
    expect(
      validatePositiveIntInput('15', fieldLabel: '后台刷新间隔'),
      isNull,
    );
  });

  test('rejects invalid minute-of-day input', () {
    expect(
      validateMinuteOfDayInput('25:00', fieldLabel: '静默时段开始时间'),
      isNotNull,
    );
    expect(
      validateMinuteOfDayInput('07:30', fieldLabel: '静默时段开始时间'),
      isNull,
    );
    expect(parseMinuteOfDayInput('07:30'), 450);
  });

  test('keeps parse fallbacks stable', () {
    expect(parsePositiveIntInput('abc', fallback: 15), 15);
    expect(parsePositiveIntInput('20', fallback: 15), 20);
  });

  test('subscription result requires backend ids', () {
    final parsed = SubscriptionResultModel.fromJson(<String, dynamic>{
      'feed_id': 'feed-1',
      'resolved_feed_url': 'https://example.com/feed.xml',
      'subscription_source': 'direct_feed',
    });

    expect(parsed.feedId, 'feed-1');
    expect(parsed.resolvedFeedUrl, 'https://example.com/feed.xml');

    expect(
      () => SubscriptionResultModel.fromJson(<String, dynamic>{
        'resolved_feed_url': 'https://example.com/feed.xml',
      }),
      throwsA(anything),
    );
  });
}
