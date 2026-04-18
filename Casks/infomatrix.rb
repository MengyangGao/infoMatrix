cask "infomatrix" do
  desc "Privacy-respecting cross-platform RSS reader"
  homepage "https://github.com/MengyangGao/infoMatrix"
  version :latest
  url "https://github.com/MengyangGao/infoMatrix/releases/latest/download/InfoMatrix-macos.zip"
  sha256 :no_check

  auto_updates true

  app "InfoMatrix.app"
end
