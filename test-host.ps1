# Test Host ICE connection fixes
# Run this script to start the host, then connect from another machine

Write-Host "Starting sscontrol host..." -ForegroundColor Green
Write-Host ""
Write-Host "Fixes applied:" -ForegroundColor Yellow
Write-Host "1. Added multiple STUN servers (Google Public STUN)"
Write-Host "2. Limited network type to IPv4 UDP"
Write-Host "3. Filtered invalid link-local address candidates"
Write-Host ""

./target/release/sscontrol.exe host
