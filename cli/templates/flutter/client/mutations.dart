import 'dart:async';

import 'package:skies_flutter/skies_flutter.dart';

MutationBoundary createMutationBoundary({
  required FutureOr<void> Function() invalidateQueries,
  required FeedbackSink feedback,
}) => MutationBoundary(
  invalidateQueries: invalidateQueries,
  feedback: feedback,
);
