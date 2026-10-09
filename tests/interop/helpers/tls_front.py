"""0.1.3: a test-only TLS front for `metagente serve --a2a`. It is a fixture, not a product feature.

`metagente serve` speaks plain http. This forwards https requests to it, the way a reverse proxy that ends
TLS would, and tells the agent so (X-Forwarded-Proto: https) so that its agent card advertises https.

Usage: python tls_front.py --listen PORT --target PORT --cert cert.pem --key key.pem
Prints `READY` when it is listening. Standard library only.
"""
import argparse
import asyncio
import ssl


async def pipe(reader, writer):
    try:
        while data := await reader.read(65536):
            writer.write(data)
            await writer.drain()
    except (ConnectionError, asyncio.CancelledError):
        pass
    finally:
        try:
            writer.close()
        except Exception:
            pass


async def handle(client_r, client_w, target):
    upstream_w = None
    try:
        head = await client_r.readuntil(b"\r\n\r\n")
        lines = head.decode("latin-1").split("\r\n")[:-2]
        length = 0
        kept = [lines[0]]
        for line in lines[1:]:
            name = line.split(":", 1)[0].strip().lower()
            if name == "content-length":
                length = int(line.split(":", 1)[1])
            if name in ("connection", "x-forwarded-proto"):
                continue
            kept.append(line)
        kept += ["X-Forwarded-Proto: https", "Connection: close"]
        body = await client_r.readexactly(length) if length else b""
        upstream_r, upstream_w = await asyncio.open_connection("127.0.0.1", target)
        upstream_w.write(("\r\n".join(kept) + "\r\n\r\n").encode("latin-1") + body)
        await upstream_w.drain()
        await pipe(upstream_r, client_w)
    except (asyncio.IncompleteReadError, ConnectionError):
        pass
    finally:
        for w in (client_w, upstream_w):
            if w is not None:
                try:
                    w.close()
                except Exception:
                    pass


async def main(args):
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
    context.load_cert_chain(args.cert, args.key)
    server = await asyncio.start_server(
        lambda r, w: handle(r, w, args.target), "127.0.0.1", args.listen, ssl=context
    )
    print("READY", flush=True)
    async with server:
        await server.serve_forever()


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--listen", type=int, required=True)
    parser.add_argument("--target", type=int, required=True)
    parser.add_argument("--cert", required=True)
    parser.add_argument("--key", required=True)
    asyncio.run(main(parser.parse_args()))
