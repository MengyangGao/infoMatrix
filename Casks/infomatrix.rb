cask "infomatrix" do
  desc "Privacy-respecting cross-platform RSS reader"
  homepage "https://github.com/MengyangGao/infoMatrix"
  version "0.1.0"
  sha256 "1a55965fa4de4ce69fb65896af90b490ee5f5bf1f8c31efa1a7286aba87a0ae9"

  url "https://github.com/MengyangGao/infoMatrix/releases/download/v#{version}/InfoMatrix-macos.zip"

  auto_updates true

  app "InfoMatrix.app"
end
