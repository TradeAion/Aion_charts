import http.server
import os

os.chdir(os.path.dirname(os.path.abspath(__file__)))
print(f"Serving from: {os.getcwd()}")
http.server.test(HandlerClass=http.server.SimpleHTTPRequestHandler, port=8080)
