import http.server
import os

os.chdir(os.path.dirname(os.path.abspath(__file__)))
print(f"Serving from: {os.getcwd()}")

class NoCacheHandler(http.server.SimpleHTTPRequestHandler):
    def end_headers(self):
        self.send_header('Cache-Control', 'no-store, no-cache, must-revalidate, max-age=0')
        self.send_header('Pragma', 'no-cache')
        self.send_header('Expires', '0')
        super().end_headers()

    def guess_type(self, path):
        if path.endswith('.wasm'):
            return 'application/wasm'
        return super().guess_type(path)

http.server.test(HandlerClass=NoCacheHandler, port=8080)
