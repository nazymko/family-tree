# THIS FILE IS AUTO-GENERATED. DO NOT MODIFY!!

# Copyright 2020-2023 Tauri Programme within The Commons Conservancy
# SPDX-License-Identifier: Apache-2.0
# SPDX-License-Identifier: MIT

-keep class app.gene_tree.desktop.* {
  native <methods>;
}

-keep class app.gene_tree.desktop.WryActivity {
  public <init>(...);

  void setWebView(app.gene_tree.desktop.RustWebView);
  java.lang.Class getAppClass(...);
  int getId();
  java.lang.String getVersion();
  int startActivity(...);
}

-keep class app.gene_tree.desktop.Ipc {
  public <init>(...);

  @android.webkit.JavascriptInterface public <methods>;
}

-keep class app.gene_tree.desktop.RustWebView {
  public <init>(...);

  void loadUrlMainThread(...);
  void loadHTMLMainThread(...);
  void evalScript(...);
}

-keep class app.gene_tree.desktop.RustWebChromeClient,app.gene_tree.desktop.RustWebViewClient {
  public <init>(...);
}
