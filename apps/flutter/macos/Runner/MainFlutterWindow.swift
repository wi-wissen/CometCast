import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  // Screens on the same network (spec 14): NWBrowser, results go to the Rust core (N50).
  private var nearbyBridge: NearbyBridge?

  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    let windowFrame = self.frame
    self.contentViewController = flutterViewController
    self.setFrame(windowFrame, display: true)

    RegisterGeneratedPlugins(registry: flutterViewController)
    nearbyBridge = NearbyBridge(messenger: flutterViewController.engine.binaryMessenger)

    super.awakeFromNib()
  }
}
