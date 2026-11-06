import SwiftUI

@main
struct CompanionApp: App {
    @State private var session = Session()

    var body: some Scene {
        WindowGroup {
            Group {
                if session.signedIn {
                    RootView()
                } else {
                    SignInView()
                }
            }
            .environment(session)
            .tint(.accent)
        }
    }
}

extension Color {
    /// The Mac app's accent, the indigo of its buttons and selection.
    static let accent = Color(red: 0.42, green: 0.45, blue: 0.95)
}
