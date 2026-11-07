import Flutter
import UIKit

@main
@objc class AppDelegate: FlutterAppDelegate, FlutterImplicitEngineDelegate {
  // Kept alive for the life of the app: they own their channels.
  private var broadcastBridge: BroadcastBridge?
  private var videoBridge: VideoBridge?
  private var linkBridge: LinkBridge?
  private var qrBridge: QrBridge?
  private var nearbyBridge: NearbyBridge?
  private var announceBridge: AnnounceBridge?

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
      // Universal Links and the system QR scanner (native-apps.md 20.4, section 8).
      linkBridge = LinkBridge(messenger: registrar.messenger())
      qrBridge = QrBridge(messenger: registrar.messenger())
      // Screens on the same network (spec 14): NWBrowser, results go to the Rust core.
      nearbyBridge = NearbyBridge(messenger: registrar.messenger())
      // The iPad in the display role announces with NWListener (decision N50).
      announceBridge = AnnounceBridge(messenger: registrar.messenger())
    }
  }
}
