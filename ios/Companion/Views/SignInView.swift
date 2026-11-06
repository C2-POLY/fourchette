import SwiftUI

/// Signing in with GitHub's device flow: a button, then the code to type on github.com.
struct SignInView: View {
    @Environment(Session.self) private var session
    @Environment(\.openURL) private var openURL

    var body: some View {
        VStack(spacing: 28) {
            Spacer()
            VStack(spacing: 10) {
                Image(systemName: "arrow.triangle.branch")
                    .font(.system(size: 52, weight: .medium))
                    .foregroundStyle(Color.accent)
                Text("Fourchette")
                    .font(.largeTitle.bold())
                Text("Your projects, pull requests and history, away from your Mac.")
                    .font(.callout)
                    .foregroundStyle(.secondary)
                    .multilineTextAlignment(.center)
            }
            .padding(.horizontal, 32)

            content
                .frame(maxWidth: 420)
                .padding(.horizontal, 24)
            Spacer()
            Text("Read only, except moving a card on a project board.")
                .font(.footnote)
                .foregroundStyle(.tertiary)
                .padding(.bottom, 12)
        }
    }

    @ViewBuilder
    private var content: some View {
        switch session.signIn {
        case .idle:
            Button {
                session.startSignIn()
            } label: {
                Label("Sign in with GitHub", systemImage: "person.crop.circle.badge.checkmark")
                    .frame(maxWidth: .infinity)
            }
            .buttonStyle(.borderedProminent)
            .controlSize(.large)
        case .requesting:
            ProgressView("Asking GitHub for a code…")
        case .waiting(let code):
            VStack(spacing: 16) {
                Text("Type this code on GitHub")
                    .font(.headline)
                Text(code.userCode)
                    .font(.system(size: 34, weight: .semibold, design: .monospaced))
                    .textSelection(.enabled)
                    .padding(.vertical, 10)
                    .padding(.horizontal, 20)
                    .background(.quaternary, in: RoundedRectangle(cornerRadius: 12))
                Button {
                    UIPasteboard.general.string = code.userCode
                    openURL(code.verificationURI)
                } label: {
                    Label("Copy and open github.com", systemImage: "safari")
                        .frame(maxWidth: .infinity)
                }
                .buttonStyle(.borderedProminent)
                .controlSize(.large)
                HStack(spacing: 8) {
                    ProgressView()
                    Text("Waiting for GitHub…")
                        .foregroundStyle(.secondary)
                }
                .font(.callout)
                Button("Cancel", role: .cancel) { session.cancelSignIn() }
            }
        case .failed(let message):
            VStack(spacing: 14) {
                Label(message, systemImage: "exclamationmark.triangle")
                    .foregroundStyle(.red)
                    .multilineTextAlignment(.center)
                Button("Try Again") { session.startSignIn() }
                    .buttonStyle(.bordered)
            }
        }
    }
}
