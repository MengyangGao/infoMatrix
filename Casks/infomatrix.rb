cask "infomatrix" do
  desc "Privacy-respecting cross-platform RSS reader"
  homepage "https://github.com/MengyangGao/infoMatrix"
  version "0.1.5"
  sha256 "5f31399f084853a28b47077ffbffc1391aa0cad1b1094fe7c570c57eebf5ec4e"

  url "https://github.com/MengyangGao/infoMatrix/releases/download/v#{version}/InfoMatrix-macos.zip"

  auto_updates true

  app "InfoMatrix.app"
end
