import 'package:flutter/widgets.dart';
import 'package:skies_flutter/skies_flutter.dart';

import '{{ stem }}_view_model.dart';

/// The render-only View for one [{{ feature }}ViewModel].
final class {{ feature }}View extends StatefulWidget {
  const {{ feature }}View({
    required this.viewModel,
    required this.ready,
    required this.loading,
    required this.empty,
    required this.failure,
    super.key,
  });

  final {{ feature }}ViewModel viewModel;
  final Widget Function(BuildContext context, List<{{ item }}> items) ready;
  final WidgetBuilder loading;
  final WidgetBuilder empty;
  final Widget Function(BuildContext context, Object error, AsyncRetry? retry) failure;

  @override
  State<{{ feature }}View> createState() => _{{ feature }}ViewState();
}

final class _{{ feature }}ViewState extends State<{{ feature }}View> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => widget.viewModel.load());
  }

  @override
  Widget build(BuildContext context) => AnimatedBuilder(
        animation: widget.viewModel,
        builder: (context, _) => ResourceBuilder<List<{{ item }}>>(
          state: widget.viewModel.state,
          loading: widget.loading,
          empty: widget.empty,
          failure: widget.failure,
          retry: widget.viewModel.load,
          ready: widget.ready,
        ),
      );
}
