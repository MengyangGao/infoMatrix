cask "infomatrix" do
  desc "Privacy-respecting cross-platform RSS reader"
  homepage "https://github.com/MengyangGao/infoMatrix"
  version "0.1.7"
  sha256 "162a6077f768a315875060aa89384cc3e57321c645df34b51d92ea867e17b4c5"

  url "https://github.com/MengyangGao/infoMatrix/releases/download/v#{version}/InfoMatrix-macos.zip"

  auto_updates true

  app "InfoMatrix.app"
end
