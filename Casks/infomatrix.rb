cask "infomatrix" do
  version "0.1.4"
  sha256 "b57a6d499af95a1177e273593ef0e1397004d53a331174c26e935d85a266e466"

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
