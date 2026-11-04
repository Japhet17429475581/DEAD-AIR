# DEAD//AIR 0.3.0: встроенный адаптер и Academy

В **0.4.0** добавлен модуль **15 / Настоящая виртуальная WPA2-сеть**: собственное Rust-ядро, Linux mac80211_hwsim, AP, клиент и наблюдатель. Открывай **REAL VIRTUAL NETWORK LAB** в Academy. Успех подтверждается scan, состояниями hostapd/wpa_supplicant, M1–M4 и ping. [Архитектура, требования и воспроизводимый тест](VIRTUAL-LABS.md). Сведения ниже описывают предыдущие версии и симуляционные кейсы.

Обновление **0.3.1** добавляет [отдельный deauth-модуль](DEAUTH.md) и увеличивает дорожную карту до 14 модулей. Ниже сохранены сведения и результаты исходной версии 0.3.0.

## Что изменилось в захвате

Отдельный USB-адаптер больше не является обязательным. Выберите встроенную или внешнюю карту в **PASSIVE LAB → Capture adapter**. Карта и Linux-драйвер должны поддерживать monitor mode; программный переключатель не добавляет отсутствующие аппаратные возможности.

Для карты с активным соединением требуется флажок **I allow temporary Wi-Fi disconnection…**. Он предупреждает о потере Интернета и удалённых сессий через эту карту. Подтверждение владения AP и согласия участников остаётся отдельным. Без согласия на отключение старт на подключённой карте блокируется и в GUI, и в backend.

В этом режиме helper сохраняет UUID исходного профиля, Managed/Autoconnect, флаг UP, PHY и индекс интерфейса в собственном root-only recovery-журнале. Через NetworkManager он отключает выбранное соединение, освобождает интерфейс, меняет **тип этой же карты** на monitor и настраивает выбранную частоту. Это не требует создания второго виртуального интерфейса, которое поддерживают не все встроенные карты.

При STOP, END SESSION и обычном закрытии collector останавливается, карта возвращается в managed mode, восстанавливаются исходные параметры и активируется сохранённый профиль через D-Bus. Успех подтверждается состоянием 100 и исходным UUID. Кадры PCAPNG, счётчики и результаты по-прежнему приходят от настоящего airodump-ng. Восстановление может занять десятки секунд; ожидание helper увеличено до 45 секунд, чтобы не прерывать нормальную повторную активацию.

Если AP недоступна, пароль/профиль изменился, устройство извлечено, индекс изменился или картой уже владеет другое соединение, приложение показывает ошибку восстановления. Оно не удаляет исходный интерфейс и не сообщает об успешном возврате без подтверждения. После аварии используйте **RESTORE STALE CAPTURE INTERFACES**. Полная проверка именно драйверного перехода и reconnect на физической карте ещё требуется.

### Отличие от check kill

Безусловный `airmon-ng check kill` **не добавлен**. По [официальной документации Aircrack-ng](https://www.aircrack-ng.org/doku.php?id=airmon-ng) он останавливает сетевые менеджеры и оставшиеся мешающие процессы, а не только выбранную карту. Чтобы не терять управление восстановлением и не прерывать другие интерфейсы, приложение использует scoped D-Bus Disconnect/Managed и ActivateConnection. Учебный модуль объясняет обе схемы и их последствия.

## Учебный режим

Откройте **ACADEMY** в меню или клавишей **0**. На рабочих страницах есть плашка **ТЕОРИЯ ЭТОГО ЭТАПА**. Она ведёт к соответствующему модулю без запуска радиоопераций.

Дорожная карта включает 13 модулей:

1. Радиоэфир, SSID/BSSID, канал, область разрешённой проверки.
2. Discovery, NetworkManager и различие объявления/проверки.
3. Managed/monitor mode, встроенная карта, отключение и возврат Wi-Fi.
4. WPA2, PMK/PTK, EAPOL M1–M4, replay counters и nonce.
5. PMKID, RSN lists, EAPOL KDE, client-less сценарий и криптолаборатория.
6. WPA3/SAE, PMF и transition mode.
7. WEP/WPS: исторические и конфигурационные риски.
8. MAC/OUI, private addresses и проверка фактического значения.
9. Словари, алгоритмы, Hashcat/OpenCL и границы результатов.
10. Enterprise/EAP, сертификаты и защитный разбор evil twin.
11. Зависимости, polkit, daemon и ограничения привилегий.
12. Memory-only/tmpfs/temporary/persistent, PANIC и ограничения shred.
13. Kismet, PCAPNG, сбор доказательств и итоговый отчёт.

Каждый модуль содержит три блока теории, первоисточники, тест из двух вопросов и лабораторию. Сначала пользователь отмечает разбор теории, затем отвечает на все вопросы правильно. Только после этого открывается лаборатория. Кейсы требуют решений на основании текущих условий; неправильный выбор объясняется, правильный переводит сценарий на следующий шаг. Повторное прохождение допускается.

Это **12 собственных двухэтапных кейсов и одна криптолаборатория**, а не автоматически запущенные виртуальные radios. Синтетические условия помечены SIMULATION и не записываются в LIVE. Ранее запущенные LIVE-задачи могут продолжаться, пока пользователь читает Academy: учебная страница сама их не запускает и не останавливает.

WifiForge приведён как внешний проект для полноценных виртуальных сетей на mininet-wifi/mac80211_hwsim. Его код не скопирован, установка и запуск из DEAD//AIR не автоматизированы. Ссылка ведёт к официальному репозиторию Black Hills InfoSec.

Прогресс хранится в памяти App и сохраняется при навигации между разделами. **EXPORT PROGRESS** создаёт отдельный JSON по запросу пользователя; **IMPORT PROGRESS** проверяет формат, известные модули и последовательность этапов. Это локальный учебный прогресс, не независимая сертификация. PANIC сбрасывает состояние в приложении; отдельно скачанный JSON находится вне каталога сессий и автоматически не удаляется.

## PMKID: точная граница реализации

До 0.3.0 PMKID-функций не было. Теперь реализованы:

- Реальные **пассивные наблюдения** PMKID из RSN association/reassociation lists и открытого EAPOL key data, привязанные к выбранному BSSID и станции.
- Проверка длины, структуры, границ полей, fragment/protected/encrypted flags; шифрованный key data не выдаётся за открытый PMKID.
- Ограниченный список уникальных наблюдений в интерфейсе. Наличие идентификатора не помечается как найденный пароль или доказанная уязвимость WPA2-PSK.
- Учебный WPA2-PSK known-answer вектор с настоящими PBKDF2-HMAC-SHA1 и HMAC-SHA1 вычислениями WebCrypto. Эталон независимо получен через Node/OpenSSL. Четыре встроенных кандидата; можно исследовать неправильный SSID, перестановку адресов и отсутствие правильного кандидата.
- Три варианта разбора: EAPOL KDE, RSN association/reassociation и модель активного client-less получения. Выбор модели не передаёт кадры.

**Активный запрос PMKID у реальной AP, импорт чужих/захваченных PMKID для подбора и автоматическая эксплуатация в эфире не реализованы.** Академия не переименовывает симуляцию в реальную атаку. В Hashcat Lab остаются существующие benchmark и известные SHA-256 vectors. WPA3-SAE и Enterprise не объявляются уязвимыми на основании WPA2-PSK формулы.

## Источники

Тексты написаны для DEAD//AIR на русском; в каждом модуле доступны прямые ссылки на документацию. Системный браузер открывается только по нажатию, обычным пользователем; привилегированный daemon для этого не используется. Внешний браузер может вести собственную историю.

- [Linux Wireless / iw](https://wireless.docs.kernel.org/en/latest/en/users/documentation/iw.html).
- [NetworkManager Device](https://networkmanager.dev/docs/api/latest/gdbus-org.freedesktop.NetworkManager.Device.html), [Settings/GetConnectionByUuid](https://networkmanager.dev/docs/api/latest/gdbus-org.freedesktop.NetworkManager.Settings.html), [ActivateConnection](https://networkmanager.dev/docs/api/latest/gdbus-org.freedesktop.NetworkManager.html).
- [Aircrack-ng airmon-ng](https://www.aircrack-ng.org/doku.php?id=airmon-ng).
- [Wireshark WLAN capture](https://wiki.wireshark.org/CaptureSetup/WLAN).
- [Hashcat PMKID research, 2018 — исторический источник](https://hashcat.net/forum/thread-7717.html), [актуальная WPA/PMKID+EAPOL wiki](https://hashcat.net/wiki/doku.php?id=cracking_wpawpa2).
- [HPE Aruba WPA3-Personal](https://arubanetworking.hpe.com/techdocs/aos/wifi-design-deploy/security/modes/wpa3-personal/).
- [NIST SP 800-153](https://csrc.nist.gov/pubs/sp/800/153/final), [polkit](https://www.freedesktop.org/software/polkit/docs/latest/polkit.8.html), [Kismet](https://www.kismetwireless.net/docs/readme/intro/kismet/).
- [WifiForge / Black Hills InfoSec](https://github.com/blackhillsinfosec/WifiForge).

## Проверки

TypeScript/Vite build; 17 frontend unit tests; браузерные проверки переходов theory → quiz → lab, сохранения прогресса, настоящего совпадения PMKID, неправильных параметров, согласия на отключение подключённой карты и прежних функций. Rust suite включает 31 успешный тест: 26 library, 1 helper, 1 isolated D-Bus roundtrip, 3 PCAP tests. D-Bus тест теперь действительно вызывает Disconnect, меняет Managed, находит сохранённый профиль, вызывает ActivateConnection и проверяет исходный UUID/состояние на тестовом сервисе. Это не аппаратный тест NetworkManager/драйвера.

Артефакты: `artifacts/academy-tests.log`, `academy-check.log`, `academy-browser-final.log`, `single-radio-browser.log`, `academy-release.log`; screenshots: `academy-roadmap.png`, `academy-pmkid.png`.

На физическом Linux-устройстве ещё нужно проверить monitor-mode переход на встроенной карте, возврат её действующего соединения, потерю AP/USB, recovery и настоящий PMKID в эфире. Системный браузер/политики desktop и установка RPM/Arch требуют проверки на соответствующих системах.

DEB 0.3.0 собран, установлен и запущен в Debian 12/WebKitGTK/Xvfb обычным пользователем; RPM собран без проверки Fedora. В нативном окне вручную пройден PMKID theory → quiz → lab и получено настоящее WebCrypto-совпадение эталона. Скриншоты: `artifacts/academy-native.png`, `academy-native-quiz.png`, `academy-native-pmkid-result.png`. Полный браузерный набор: **14/14**, frontend unit: **17/17**, Rust: **31**. Daemon установленной версии прошёл проверки повторного использования PID, UID/token и отказа произвольной операции (`artifacts/academy-broker.log`).

Пакеты: [DEB 0.3.0](../artifacts/releases/DEAD%20AIR_0.3.0_amd64.deb), [RPM 0.3.0](../artifacts/releases/DEAD%20AIR-0.3.0-1.x86_64.rpm). `SHA256SUMS` находится рядом с ними.

В учебном режиме клавиша R не запускает новое LIVE-сканирование, даже если сессия аудита уже открыта. Эта изоляция проверена отдельным IPC contract test вместе с согласием на отключение карты. Предварительно запущенные реальные задачи при навигации не прерываются автоматически.
