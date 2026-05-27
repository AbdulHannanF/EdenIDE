"""Tiny dev server — avoids file:// CORS block when Babel loads .jsx files.
Run: python serve.py  (then open http://localhost:3000)
"""
import http.server, socketserver, os

PORT = 3000
os.chdir(os.path.dirname(os.path.abspath(__file__)))

class Handler(http.server.SimpleHTTPRequestHandler):
    def log_message(self, fmt, *args):
        print(f"  {args[0]}  {args[1]}")

with socketserver.TCPServer(("", PORT), Handler) as httpd:
    print(f"EDEN design server → http://localhost:{PORT}")
    print("Ctrl+C to stop.")
    httpd.serve_forever()
