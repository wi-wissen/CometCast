import Flutter
import UIKit

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  // Kept alive for the life of the app: they own their channels.
  private var broadcastBridge: BroadcastBridge?
  private var videoBridge: VideoBridge?

  override func application(
    _ application: UIApplication,
    didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey: Any]?
  ) -> Bool {
    return super.application(application, didFinishLaunchingWithOptions: launchOptions)
  }

  func didInitializeImplicitFlutterEngine(_ engineBridge: FlutterImplicitEngineBridge) {
    GeneratedPluginRegistrant.register(with: engineBridge.pluginRegistry)
    // Comet Cast's own channels (docs/ios.md). UNCERTAIN: `registrar(forPlugin:)` on the
    // implicit engine's plugin registry is the documented way to get a messenger and the
    // texture registry; never compiled.
    if let registrar = engineBridge.pluginRegistry.registrar(forPlugin: "CometCastBridges") {
      broadcastBridge = BroadcastBridge(messenger: registrar.messenger())
      videoBridge = VideoBridge(messenger: registrar.messenger(), textures: registrar.textures())
    }
  }
}
