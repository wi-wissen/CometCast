import Flutter
import UIKit

class SceneDelegate: FlutterSceneDelegate {
  // Universal Links (native-apps.md 20.4): a cold start brings the link with the connection
  // options, a running app gets it as a user activity. LinkBridge hands it to Dart.
  override func scene(
    _ scene: UIScene, willConnectTo session: UISceneSession,
    options connectionOptions: UIScene.ConnectionOptions
  ) {
    LinkBridge.launch(with: connectionOptions.userActivities)
    super.scene(scene, willConnectTo: session, options: connectionOptions)
  }

  override func scene(_ scene: UIScene, continue userActivity: NSUserActivity) {
    LinkBridge.deliver(userActivity)
    super.scene(scene, continue: userActivity)
  }
}
