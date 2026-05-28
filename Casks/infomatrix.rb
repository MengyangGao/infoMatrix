cask "infomatrix" do
  desc "Privacy-respecting cross-platform RSS reader"
  homepage "https://github.com/MengyangGao/infoMatrix"
  version "0.1.4"
  sha256 "2ca348b4b977b4d1c54a16fdfde3b6a44a98428bad54d811b166b1727c5802ac"

  url "https://github.com/MengyangGao/infoMatrix/releases/download/v#{version}/InfoMatrix-macos.zip"

  auto_updates true

  app "InfoMatrix.app"
end
