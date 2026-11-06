import Foundation

/// GitHub's device flow: the app shows a code, the person types it on github.com, and the app
/// polls until GitHub hands it a token. No secret is involved, only the OAuth App's client id.
/// https://docs.github.com/apps/oauth-apps/building-oauth-apps/authorizing-oauth-apps#device-flow
enum DeviceFlow {
    /// What the companion reads: the repositories, the organisations', and the Projects (the
    /// board's moves write them).
    static let scopes = "repo read:org project"

    struct Code: Decodable, Equatable, Sendable {
        let deviceCode: String
        let userCode: String
        let verificationURI: URL
        let expiresIn: Int
        let interval: Int

        enum CodingKeys: String, CodingKey {
            case deviceCode = "device_code", userCode = "user_code", verificationURI = "verification_uri", expiresIn = "expires_in", interval
        }
    }

    /// One answer while polling for the token.
    enum Poll: Equatable, Sendable {
        case token(String)
        /// Not typed yet: ask again after the interval.
        case pending
        /// Asked too often: GitHub gives the new interval.
        case slowDown(Int)
        case failed(String)
    }

    enum Failure: LocalizedError {
        case noClientID
        case github(String)

        var errorDescription: String? {
            switch self {
            case .noClientID: "This build has no GitHub client id. Set GITHUB_CLIENT_ID in ios/Config.xcconfig."
            case .github(let message): message
            }
        }
    }

    /// The OAuth App's client id, from the Info.plist (Config.xcconfig fills it in).
    static var clientID: String? {
        let id = Bundle.main.object(forInfoDictionaryKey: "GitHubClientID") as? String
        return id.flatMap { $0.isEmpty || $0.hasPrefix("$(") ? nil : $0 }
    }

    static func requestCode(clientID: String) async throws -> Code {
        let data = try await post("https://github.com/login/device/code", ["client_id": clientID, "scope": scopes])
        if let error = errorMessage(data) {
            throw Failure.github(error)
        }
        return try JSONDecoder().decode(Code.self, from: data)
    }

    static func poll(clientID: String, deviceCode: String) async throws -> Poll {
        let data = try await post("https://github.com/login/oauth/access_token", [
            "client_id": clientID, "device_code": deviceCode, "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
        ])
        return parsePoll(data)
    }

    /// GitHub answers 200 in every case, with either `access_token` or `error`.
    static func parsePoll(_ data: Data) -> Poll {
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else { return .failed("GitHub sent an answer the app can't read.") }
        if let token = json["access_token"] as? String { return .token(token) }
        switch json["error"] as? String {
        case "authorization_pending": return .pending
        case "slow_down": return .slowDown(json["interval"] as? Int ?? 10)
        case "expired_token": return .failed("The code expired. Ask for a new one.")
        case "access_denied": return .failed("The sign-in was cancelled on GitHub.")
        default: return .failed(json["error_description"] as? String ?? "GitHub refused the sign-in.")
        }
    }

    private static func errorMessage(_ data: Data) -> String? {
        guard let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any], json["error"] != nil else { return nil }
        return json["error_description"] as? String ?? json["error"] as? String
    }

    private static func post(_ url: String, _ form: [String: String]) async throws -> Data {
        var request = URLRequest(url: URL(string: url)!)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("application/x-www-form-urlencoded", forHTTPHeaderField: "Content-Type")
        var components = URLComponents()
        components.queryItems = form.map { URLQueryItem(name: $0.key, value: $0.value) }
        request.httpBody = components.percentEncodedQuery?.data(using: .utf8)
        let (data, _) = try await URLSession.shared.data(for: request)
        return data
    }
}
