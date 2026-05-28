cask "infomatrix" do
  desc "Privacy-respecting cross-platform RSS reader"
  homepage "https://github.com/MengyangGao/infoMatrix"
  version "0.1.6"
  sha256 "1516953a6bcdac7a3322a384cf9cf69a267ed64239c84fdfdb9a018f2d4403fd"

  url "https://github.com/MengyangGao/infoMatrix/releases/download/v#{version}/InfoMatrix-macos.zip"

  auto_updates true

  app "InfoMatrix.app"
end
