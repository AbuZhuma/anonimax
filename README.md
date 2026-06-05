# anonimax

Модульная анти-детект CLI-панель: запросы с полной эмуляцией браузера
(TLS/JA3, HTTP2, заголовки, UA) и сменой IP через Tor или прокси.

## Установка на новом устройстве

Пример для Arch. Для Debian/Ubuntu замени шаг 1 на:
`sudo apt install -y git cargo cmake clang build-essential tor`

```bash
# 1. Зависимости + Tor
sudo pacman -S --needed git rust cmake clang gcc tor

# 2. Запустить Tor
sudo systemctl enable --now tor

# 3. Собрать и установить
git clone <репозиторий> anonimax
cd anonimax
cargo install --path .

# 4. PATH (только если `anonimax: command not found`)
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.bashrc && source ~/.bashrc

# 5. Запуск
anonimax
```

Дальше — в самой панели:

```
use anon
tor on
ip
```

## Команды панели

| Команда        | Что делает        |
|----------------|-------------------|
| `modules`      | список модулей    |
| `use <module>` | войти в модуль    |
| `back`         | выйти из модуля   |
| `help`         | справка по модулю |
| `exit`         | выход             |

## Модуль `anon`

```
id                      # текущая настройка (браузер, маршрут, прокси)
ip                      # какой IP/страну/ISP видит сервер
test                    # показать свой TLS-отпечаток (tls.peet.ws)

send <url>                       # GET
send POST <url> <тело>           # любой метод: GET/POST/PUT/PATCH/DELETE/HEAD/OPTIONS
send PUT https://api/x '{"a":1}' # пример с телом
header add X-Token abc123        # заголовок ко всем запросам
header list | header clear       # показать / очистить заголовки

browser list            # доступные браузеры/устройства
browser firefox         # закрепить браузер
rotate                  # сменить браузер на случайный
auto on|off             # менять браузер перед каждым запросом

tor on|off              # маршрут через Tor (новый exit IP на каждый запрос)
tor ip                  # проверить текущий exit IP
tor new                 # сбросить все цепочки Tor (NEWNYM)

proxy add socks5h://login:pass@host:port   # добавить прокси
proxy load proxies.txt                     # загрузить список (по строке, # - коммент)
proxy list                                 # показать пул
proxy mode rotate|random|off               # как ротировать IP
proxy clear                                # очистить пул
```

## Модуль `gateway` — запросы из своего кода через anonimax

Поднимает локальный HTTP-сервер. Любой твой код (Python, Node, и т.д.) шлёт
простой JSON, а anonimax выполняет запрос с эмуляцией браузера + текущим
маршрутом (Tor/прокси из модуля `anon`) и возвращает ответ.

```
use gateway
start                   # поднять на 127.0.0.1:8888 (можно: start 9000)
status
stop
```

Запрос из кода:

```python
import requests
r = requests.post("http://127.0.0.1:8888/request", json={
    "method": "GET",
    "url": "https://api.ipify.org?format=json",
    "headers": {"X-Token": "abc"},   # необязательно
    "body": None                     # необязательно
})
```

## Модуль `system` — весь трафик устройства через Tor

Меняет IP у **всего** (браузер, приложения, скрипты) — заворачивает весь TCP+DNS
в Tor через firewall. Нужен root (спросит пароль sudo).

```
use system
system status          # проверить, идёт ли весь трафик через Tor
system on              # завернуть всё устройство в Tor
system off             # вернуть как было
```