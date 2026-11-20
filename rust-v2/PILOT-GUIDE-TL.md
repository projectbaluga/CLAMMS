# CLAMMS Rust V2 — Pilot at Deployment Checklist

Gamitin muna ito sa isang laboratory workstation at sa computer ng custodian. Huwag muna sabay-sabay i-deploy sa anim na laboratoryo.

## 1. Bago mag-install

- [ ] May backup na ang kasalukuyang PHP/legacy inventory at database.
- [ ] Nakumpirma ang Windows version at administrator access sa custodian PC.
- [ ] May fixed/reserved LAN IP ang custodian PC.
- [ ] Gumagana ang VPN para sa remote access; walang public port forwarding.
- [ ] Nakahanda ang unique administrator username/password.
- [ ] Nakahanda ang random agent secret na hindi bababa sa 32 characters.
- [ ] Naka-allow lamang ang TCP 8080 mula sa school LAN at VPN subnet sa Windows Firewall.

## 2. Build at install ng server

1. Sa rust-v2 folder, patakbuhin ang cargo build --release.
2. I-verify ang target\release\clamms-server.exe.
3. Kopyahin ang .env.example bilang .env at palitan ang placeholder values.
4. Patakbuhin ang install-server.ps1 gamit ang elevated PowerShell.
5. I-edit ang C:\ProgramData\CLAMMS\Server\.env; huwag maglagay ng totoong secret sa Git.
6. Simulan ang task: Start-ScheduledTask -TaskName "CLAMMS Server".
7. Buksan ang http://127.0.0.1:8080 sa custodian PC.
8. Mag-login at tiyaking na-create ang administrator. Alisin ang bootstrap password sa .env pagkatapos ma-create ang unang account.

## 3. Initial inventory

Ihanda muna ang spreadsheet o printed inventory bilang source of truth. Gumamit ng unique asset tag, halimbawa:

- LAB01-PC001 hanggang LAB01-PC0NN
- LAB02-PC001 hanggang LAB02-PC0NN
- hanggang LAB06

I-record ang laboratory, asset tag, category, brand/model, serial number, hostname, IP address kung static/reserved, acquisition date, warranty end at physical condition. Iwasang hulaan ang serial number o purchase date; iwanang blanko kung hindi alam. I-double-check ang duplicate tags bago i-encode. Sa kasalukuyang dashboard, manual record entry ang available; wala pang bulk CSV import.

## 4. One-workstation agent pilot

1. Gumawa muna ng asset record sa dashboard. Dapat eksaktong tugma ang asset tag.
2. Sa rust-v2\agent, patakbuhin ang cargo build --release.
3. Kopyahin ang agent\.env.example bilang agent\.env at ilagay ang server URL, parehong agent secret, at exact asset tag.
4. Patakbuhin ang agent\install-agent.ps1 gamit ang elevated PowerShell.
5. I-edit ang C:\ProgramData\CLAMMS\Agent\.env at tiyaking walang placeholder values.
6. Simulan: Start-ScheduledTask -TaskName "CLAMMS Workstation Monitoring Agent".
7. Hintayin ang susunod na heartbeat (default 60 seconds), pagkatapos ay tingnan ang Monitoring tab.
8. I-restart ang test PC at tiyaking kusang bumabalik ang agent task.
9. I-test ang maling asset tag at maling secret sa controlled test; dapat tanggihan ng server ang heartbeat.

## 5. Backup at restore drill

- [ ] Mag-download ng backup gamit ang administrator dashboard.
- [ ] Kopyahin ang backup sa encrypted/off-device storage.
- [ ] Patakbuhin ang backup-database.ps1 para sa offline backup at SHA-256 checksum.
- [ ] Sa hiwalay na test PC o test installation, patakbuhin ang restore-database.ps1 -BackupPath <file>.
- [ ] I-verify ang login, laboratories, assets, maintenance records at monitoring history pagkatapos ng test restore.
- [ ] Itala ang petsa, operator, backup filename, checksum at resulta ng restore drill.

Huwag mag-restore sa production hangga't hindi malinaw ang napiling backup at may pre-restore backup ng kasalukuyang database.

## 6. Acceptance checks bago ang rollout

- [ ] Nakakapasok ang authorized custodian at viewer; hindi nakakagawa ng changes ang viewer.
- [ ] Gumagana ang asset/laboratory create at update.
- [ ] Gumagana ang maintenance create/update at status changes.
- [ ] Gumagana ang user creation, deactivation at password reset.
- [ ] Lumalabas ang agent telemetry at tama ang asset tag.
- [ ] Gumagana ang CSV exports at nabubuksan sa spreadsheet.
- [ ] Nakikita ng administrator ang audit history at nakakapag-download ng DB backup.
- [ ] Nakakakonekta ang remote custodian sa VPN lamang.
- [ ] Nakumpleto ang restore drill.
- [ ] Walang critical error sa server/agent logs sa loob ng napagkasunduang pilot period.

## 7. Rollback

Kung may problema, ihinto ang rollout. Itigil ang agent scheduled task sa pilot PC at ang server task sa custodian PC. Panatilihin ang database at .env files; huwag burahin ang data folder. Bumalik muna sa validated legacy workflow at ayusin ang problema sa test copy. Huwag i-overwrite ang legacy database gamit ang Rust V2 data nang walang reviewed migration plan.

## Known limitations

Ang Rust V2 ay development/pilot build. Wala pang bulk inventory import, PDF reports, alert notification workflow, complete role policy review, formal rate limiting, at independent penetration/security review. Ang Task Scheduler scripts ay startup tasks, hindi native Windows services. Kailangan pa rin ng aktuwal na Windows pilot at successful CI build bago production use.
