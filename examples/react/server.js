const http = require('http')
const fs = require('fs')
const path = require('path')

const port = process.env.PORT || 3000

const server = http.createServer((req, res) => {
  let p = req.url
  if (p === '/') p = '/index.html'
  const file = path.join(__dirname, p)
  fs.readFile(file, (err, data) => {
    if (err) {
      res.statusCode = 404
      return res.end('Not found')
    }
    const ext = path.extname(file)
    const mime = ext === '.js' ? 'application/javascript' : 'text/html'
    res.setHeader('Content-Type', mime)
    res.end(data)
  })
})

server.listen(port, () => console.log(`Static server for example listening at http://localhost:${port}`))
