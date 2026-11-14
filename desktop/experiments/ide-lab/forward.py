import asyncio,os
async def connection(reader,writer,port):
 try:
  remote_reader,remote_writer=await asyncio.open_connection(os.environ['UPSTREAM'],port)
  async def pipe(src,dst):
   try:
    while data:=await src.read(65536):
     dst.write(data);await dst.drain()
   finally: dst.close()
  await asyncio.gather(pipe(reader,remote_writer),pipe(remote_reader,writer))
 except (OSError,ConnectionError):writer.close()
async def main():
 servers=[]
 for port in [int(os.environ['IDE_PORT']),3001]:
  async def handler(r,w,p=port):await connection(r,w,p)
  servers.append(await asyncio.start_server(handler,'0.0.0.0',port))
 await asyncio.gather(*(s.serve_forever() for s in servers))
asyncio.run(main())
