import http.server
import socketserver
import webbrowser
import sys
import os

PORT = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
DIRECTORY = os.path.dirname(os.path.abspath(__file__))

class Handler(http.server.SimpleHTTPRequestHandler):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, directory=DIRECTORY, **kwargs)

    def end_headers(self):
        self.send_header("Cache-Control", "no-cache")
        super().end_headers()

candidates = [PORT] + [p for p in (8000, 8080, 8888, 5500, 5678, 3000) if p != PORT]
server = None
for port in candidates:
    try:
        server = socketserver.ThreadingTCPServer(("127.0.0.1", port), Handler)
        break
    except OSError:
        continue

if server is None:
    print("Aucun port disponible. Fermeture du serveur.")
    input("Appuyez sur Entrée pour quitter...")
    sys.exit(1)

url = f"http://127.0.0.1:{server.server_address[1]}"
print(f"O'Turkish Kebab en ligne : {url}")
try:
    webbrowser.open(url)
except Exception:
    pass
try:
    server.serve_forever()
except KeyboardInterrupt:
    print("\nServeur arrêté.")

