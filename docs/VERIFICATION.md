# DEAD//AIR 0.1.0 — отчёт проверки

Это исторический отчёт 0.1.0. Новые функции и проверки 0.2.0 описаны в [PRIVACY.md](PRIVACY.md).

Дата: 30 октября 2026. Это рабочая Linux-сборка с реализованным backend, **не заявление о завершённой аппаратной приёмке**.

## Готовые артефакты

- [DEB x86_64](../artifacts/releases/DEAD%20AIR_0.1.0_amd64.deb) — собран и установлен в Debian 12; установленный GUI запущен и проверен.
- [RPM x86_64](../artifacts/releases/DEAD%20AIR-0.1.0-1.x86_64.rpm) — собран; установка и запуск на Fedora ещё не проверены.
- [SHA256SUMS](../artifacts/releases/SHA256SUMS) — контрольные суммы этих файлов.
- [Инструкции Linux](LINUX.md) — Ubuntu, Debian, Fedora и Arch, сборка, зависимости, тестирование и аппаратная приёмка.

Бинарники собраны в Debian 12. Не предполагается автоматическая ABI-совместимость с любой версией Linux. Arch-рецепт предоставлен в исходниках, готовый Arch-пакет в этой среде не собирался. Windows-версия радио-backend не реализована.

## Что реализовано и чем проверено

| Компонент | Реализация | Проведённая проверка |
|---|---|---|
| GUI и ASCII-череп | CRT-панели, сетка 72×48, четыре положения челюсти, отдельные глаза, реакции и редкое декоративное поведение | 10 модульных тестов; браузерная геометрия, анимация, reduced motion; визуальная проверка Chrome и настоящего WebKitGTK |
| Разделение LIVE/DEMO | LIVE использует IPC, без автоматической подстановки вымышленных результатов | Браузерные проверки и нативный запуск без радиоустройства: нулевые наблюдения, реальные ошибки служб |
| Первый запуск | Автопроверка, список пакетов, установка через fixed-profile helper и polkit, повторная проверка | Настоящий dependency plan в Tauri; ошибка отсутствующего polkit/D-Bus видна; реальный helper успешно установил пакеты через apt в контейнере |
| Ошибки установки | Видимое сообщение, повторная попытка, отсутствующий helper блокирует установку | IPC contract tests с явно подставленным тестовым backend; это не тест системного диалога авторизации |
| NetworkManager | D-Bus discovery, ожидание LastScan, свойства AP, фильтрация данных | Интеграционный тест через настоящий изолированный D-Bus и тестовый сервис; физическое сканирование не проверено |
| Aircrack-ng | `airmon-ng` report, `airodump-ng` capture, проверка адаптера, отдельный monitor interface, журнал и восстановление | Linux compilation; реальные программы обнаружены; радиозахват и восстановление драйвера на оборудовании не проверены |
| Пакеты | PCAP tail reader, radiotap/802.11, EAPOL-корреляция, PCAPNG export | Тесты усечённых/чужих кадров, последовательностей, replay/nonce, порядка байтов, timestamp units, частичной записи и длин блоков |
| Сессии | Memory-only / temporary / persistent, приватные каталоги, manifests, locks, recovery | Rust-тесты хранения/очистки/ссылок; настоящая temporary-сессия создана установленным GUI и удалена при штатном закрытии |
| Hashcat Lab | Реальный SHA-256 benchmark и generated known-answer job, отмена, фильтрация вывода, удаление входных файлов | Два интеграционных теста на реальном Hashcat 6.2.6 + PoCL CPU: оба прошли; известный пароль восстановлен, benchmark дал скорость, stop завершил процесс |
| Kismet | Опциональная проверка локального status endpoint с token | Код собран; с настоящим сервером Kismet не проверялся; ingestion отсутствует |
| Упаковка | Tauri DEB/RPM, helper, polkit policy; Arch local-build recipe | DEB/RPM созданы, содержимое DEB проверено, DEB установлен; Fedora/Arch installation не проверена |

## Результаты автоматических проверок

- `npm run build`: production frontend собран.
- `npm test`: **10/10**.
- `npm run test:e2e`: **8/8**, Google Chrome на Windows. Включает два явно обозначенных IPC contract tests установщика.
- `cargo test --locked --no-default-features`: **20 успешных тестов** — 16 модульных, 1 D-Bus integration, 3 PCAP tail integration. Два аппаратно-зависимых Hashcat tests здесь намеренно ignored.
- Отдельный запуск `hashcat_real -- --ignored --test-threads=1`: **2/2** на настоящем CPU OpenCL runtime; 140 секунд.
- `cargo check --locked --all-targets`: успешно.
- Release GUI/helper с `tauri/custom-protocol`: собраны, production assets встроены.
- Tauri bundler: DEB и RPM созданы; `dpkg -i` завершился успешно.

Логи: `artifacts/linux-tests.log`, `artifacts/hashcat-real.log`, `artifacts/desktop-check.log`, `artifacts/linux-release.log`, `artifacts/linux-bundle.log`, `artifacts/package-validation.log`, `artifacts/installer-real.log`.

## Нативное окно и завершение

Установленный `/usr/bin/dead-air` запускался обычным пользователем в Debian 12/WebKitGTK/Xvfb. Проверены настоящий IPC, создание временной сессии и завершение через `WM_DELETE_WINDOW`. Сессия `0de16069-1ed7-4c14-93a6-8909126d7d46` удалена; две старые сессии после аварийных остановок не затронуты. Процесс приложения завершился. Сопоставление сохранено в `artifacts/native-before-close.log` и `artifacts/native-after-close.log`.

Ранний тест через `xdotool windowclose` принудительно уничтожил X window и вызвал GDK BadDrawable; это не штатный запрос закрытия. После этого использован отдельный скрипт `scripts/verify-window-close.py`, отправляющий корректный протокол. Аварийные сессии оставались доступны для recovery, без заявления о выполненной очистке.

В контейнерном harness отключён внутренний sandbox WebKit, которому недоступны нужные user namespaces Docker. Этот override отсутствует в приложении и установленном desktop entry. В контейнере также нет полноценного accessibility bus; проверка с настоящим desktop/accessibility stack ещё нужна.

Скриншоты: [нативный GUI](../artifacts/linux-dashboard.png), [нативная сессия](../artifacts/linux-session.png), [первый запуск](../artifacts/linux-native.png), [реальная ошибка polkit](../artifacts/linux-setup-error.png), [четыре положения челюсти](../artifacts/skull-poses.png). Визуально сверены форма черепа, самостоятельная челюсть, сетка, зелёный люминофор и красные акценты относительно пользовательских референсов.

## Что осталось проверить или реализовать

1. Реальное discovery и passive capture на Wi-Fi-адаптерах разных драйверов; переход/возврат monitor mode, USB hot-unplug, конкурентная смена состояния, аварийное восстановление. Это главный оставшийся приёмочный этап.
2. Системный диалог polkit и полная цепочка GUI → авторизация → пакетный менеджер на обычных рабочих столах Ubuntu/Debian/Fedora/Arch. Успешная apt-установка проверена прямым запуском root-helper в изолированном контейнере; авторизация пользователя при этом не проверялась.
3. Fedora RPM installation, Arch packaging/install, distro-specific зависимости, X11/Wayland, аудио и GPU/CUDA/OpenCL на реальном компьютере.
4. Проверка PCAPNG из настоящего радиоэфира в Wireshark; поведение на WPA3. Нынешняя EAPOL-корреляция не равна криптографической проверке handshake.
5. Интеграция с настоящим Kismet server. Сейчас реализован только status/authentication check.
6. Импорт захваченных wireless hashes и их подбор в Hashcat не реализованы: лаборатория ограничена benchmark и известным SHA-256 вектором. Произвольные GPU-драйверы, словари, AppImage и подписанное автообновление приложения не предоставлены.

Подробные ограничения хранения, tmpfs/swap и аварийной очистки: [ARCHITECTURE.md](ARCHITECTURE.md). Вне DEMO отсутствующие устройства, службы и зависимости не превращаются в вымышленные успешные операции.
