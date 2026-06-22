cask "infomatrix" do
  version "0.1.5"
  sha256 "7361e309d9f1187a5589f9b0076fc24d4d3c020415bd4a5e5298b280e7c238b9"

  url "https://github.com/MengyangGao/infoMatrix/releases/download/v#{version}/InfoMatrix-macos.zip"
  name "InfoMatrix"
  desc "Privacy-respecting cross-platform RSS reader"
  homepage "https://github.com/MengyangGao/infoMatrix"

  livecheck do
    url :url
    strategy :github_latest
  end

  depends_on macos: :sonoma

  app "InfoMatrix.app"

  zap trash: [
    "~/Library/Application Support/InfoMatrix",
    "~/Library/Caches/com.infomatrix.app",
    "~/Library/Caches/InfoMatrix",
    "~/Library/HTTPStorages/com.infomatrix.app",
    "~/Library/Logs/InfoMatrix",
    "~/Library/Preferences/com.infomatrix.app.plist",
    "~/Library/Saved Application State/com.infomatrix.app.savedState",
    "~/Library/WebKit/com.infomatrix.app",
  ]
end
