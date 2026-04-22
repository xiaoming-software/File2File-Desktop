package main

import (
	"fmt"
	"io"
	"net"
	"sync"
	"time"
)

// 用于连接relay的tcp连接,内部实现自动重连
type RelayConn struct {
	channel       *RelayChannel
	response_pool *sync.Map
	host          string
	port          string
	conn          *net.TCPConn
	status        bool // 连接是否正常?
	closed        bool
	badCount      int   // 心跳错误次数
	lastActive    int64 // 最后活跃时间,包含数据包、心跳包,如果一直在发业务数据包, 就不需要单独发送心跳包,当空闲3秒后就发心跳包
	mu            sync.Mutex
}

func newRelayConn(channel *RelayChannel, host string, port string) *RelayConn {
	relayConn := &RelayConn{channel: channel, host: host, port: port, response_pool: &sync.Map{}}
	relayConn.doConn()

	// 心跳自动连接server,断网重连
	go relayConn.startHeartbeat()

	// 读取tcp的输出流
	go relayConn.read()

	return relayConn
}

// 关闭旧连接,重新开启新连接,每次relay连接都需要重新握手
func (c *RelayConn) doConn() {
	c.status = false
	if c.conn != nil {
		c.conn.Close()
		c.conn = nil
	}

	// 连接server
	targetConn, err := net.DialTimeout("tcp", fmt.Sprintf("%s:%s", c.host, c.port), 10*time.Second)
	if err != nil {
		// log.Println("连接relay异常:", err)
		return
	}
	tcpConn, ok := targetConn.(*net.TCPConn)
	if !ok {
		// log.Println("连接relay失败")
		targetConn.Close()
		return
	}
	tcpConn.SetKeepAlive(true)
	tcpConn.SetNoDelay(true)

	// relay 握手: 4个字节描述后续握手数据的长度,握手加密数据  sessionId=?,A=?,B=?,token=?,me=?
	obj := map[string]interface{}{
		"sessionId": fmt.Sprintf("%d", c.channel.session.id), // 传字符串
		"A":         c.channel.session.A,
		"B":         c.channel.session.B,
		"me":        c.channel.session.me,
		"token":     token,
	}
	param := toJsonString(obj)
	d := []byte(param)
	// 加密数据包
	cip, e := aesUtil.EncryptBytes(d, key)
	if e != nil {
		// log.Println("加密错误:", e)
		c.failure()
		tcpConn.Close()
		return
	}

	data := []byte{}
	data = append(data, uint32ToByteArray(uint32(len(cip)))...) //业务包大小4个字节
	data = append(data, cip...)                                 //业务包
	if err := writeAll(tcpConn, data); err != nil {
		c.failure()
		tcpConn.Close()
		return
	}

	// 读取握手结果，必须用 ReadFull 防止半包导致误判失败
	header := make([]byte, 4)
	n, e := io.ReadFull(tcpConn, header)
	if e == nil && n == 4 {
		// 读取后续解密握手参数
		size := byteArrayToUin32(header)
		dr := make([]byte, size)
		n, e = io.ReadFull(tcpConn, dr)
		// log.Println("size=", size, " n=", n)
		if e == nil && n == int(size) {
			// 解密
			r, e1 := aesUtil.DecryptBytes(dr, key)
			if e1 != nil {
				// log.Println("解密错误:", e)
				c.failure()
				tcpConn.Close()
				return
			} else {
				if len(r) == 1 && r[0] == 200 {
					// 握手成功，先挂上 conn 再标记可用，避免“可用但不可写”的竞态。
					c.conn = tcpConn
					c.ok()
				} else {
					c.failure()
					tcpConn.Close()
					return
				}
			}
		}

	} else {
		// log.Println("relay 握手错误:", e)
		c.failure()
		tcpConn.Close()
		return
	}

}

// 定时心跳3次失败的重连
func (c *RelayConn) startHeartbeat() {
	// 没登录成功之前发送登录请求,后面就发送心跳请求
	for {
		if c.channel.session.closed || c.closed {
			// 会话关闭
			break
		}

		if c.conn == nil {
			c.doConn()
		}
		if c.conn != nil {
			//log.Println("最近活跃 : ", (Now() - c.lastActive))
			if (Now() - c.lastActive) < 6000 {
				time.Sleep(time.Second)
				continue
			}

			// 业务数据包结构设计:第一个字节表示类型0是心跳、1字节数据、2文件数据...,4个字节是channel id,业务完整字节数据
			// 业务的心跳只有一个字节:0 后面不会有其他数据
			data := []byte{0}

			r, e := c.request(data, 3000)
			if e != nil {
				// log.Println("relay 心跳失败")
				// log.Println(c.channel.session.me, " relay 连接失败:", e, c.host)
				c.failure()
			} else {
				if len(r) == 1 && r[0] == 200 {
					// log.Println(c.channel.session.me, "relay 心跳成功")
					// 心跳成功
					c.ok()
				} else {
					// log.Println(c.channel.session.me, "relay 心跳失败")
					c.failure()
				}
			}

		}

		time.Sleep(time.Second * 3) // 3秒一个心跳
	}

}

// 请求等待响应
func (c *RelayConn) request(d []byte, timeOut int64) ([]byte, error) {
	// log.Println("request ", d) // 注释掉高频日志以减少性能损耗
	c.lastActive = Now()
	if c.conn == nil {
		return nil, fmt.Errorf("relay conn not ready")
	}

	// 1. 先压缩
	//var buf bytes.Buffer
	//w := zlib.NewWriter(&buf)
	//if _, err := w.Write(da); err != nil {
	//	return nil, err
	//}
	//w.Close()
	//d := buf.Bytes()

	// 加密数据包
	cip, e := aesUtil.EncryptBytes(d, c.channel.session.sessionPwd)
	if e != nil {
		// log.Println("加密错误:", e)
		return nil, e
	}
	//log.Println("加密前size=", len(da), " 加密后size=", len(cip))

	requeustId := randId()
	data := []byte{1}                                           // request数据体
	data = append(data, uint32ToByteArray(requeustId)...)       // 四个字节id
	data = append(data, uint32ToByteArray(uint32(len(cip)))...) // 业务包大小4个字节
	data = append(data, cip...)                                 // 业务包

	// 先注册等待通道，再发包，避免响应过快导致竞态丢响应。
	respChan := make(chan []byte, 1)
	c.response_pool.Store(requeustId, respChan)
	defer c.response_pool.Delete(requeustId)

	// 安全 写出去
	c.write(data)

	// --- 优化开始 ---
	select {
	case resData := <-respChan:
		return resData, nil //新方式没有带时间戳了
		// 收到数据
	case <-time.After(time.Duration(timeOut) * time.Millisecond):
		// 超时 (假设 timeOut 单位是毫秒)
		return nil, fmt.Errorf("Time out request")
	}
	// --- 优化结束 ---
}

// 响应确认包返回200
func (c *RelayConn) responseStatus(requestId uint32) {
	//log.Println("已经回复确认")
	// 加密数据包 业务确认只返回0
	cip, e := aesUtil.EncryptBytes([]byte{uint8(200)}, c.channel.session.sessionPwd)
	if e != nil {
		// log.Println("response加密错误:", e)
		return
	}

	data := []byte{0}                                           // 响应包
	data = append(data, uint32ToByteArray(requestId)...)        //四个字节id
	data = append(data, uint32ToByteArray(uint32(len(cip)))...) //业务包大小4个字节
	data = append(data, cip...)

	// 安全 写出去
	c.write(data)
}

// 响应数据(内部协议使用)
func (c *RelayConn) responseData(requestId uint32, d []byte) {
	d = append([]byte{uint8(200)}, d...)
	//log.Println("已经回复确认")
	// 加密数据包 业务确认只返回0
	cip, e := aesUtil.EncryptBytes(d, c.channel.session.sessionPwd)
	if e != nil {
		// log.Println("response加密错误:", e)
		return
	}

	data := []byte{0}                                           // 响应包
	data = append(data, uint32ToByteArray(requestId)...)        //四个字节id
	data = append(data, uint32ToByteArray(uint32(len(cip)))...) //业务包大小4个字节
	data = append(data, cip...)

	// 安全 写出去
	c.write(data)
}

// 线程安全的写出
func (c *RelayConn) write(data []byte) {
	c.mu.Lock()
	defer c.mu.Unlock()
	if c.conn != nil {
		//log.Println("relay 写出数据:", data)
		if err := writeAll(c.conn, data); err != nil {
			c.restart()
		}
	}
}

// 请求等待响应 request:1,四个字节id,业务包大小4个字节,业务包...   response:0,四个字节id,响应包大小4个字节,响应包...
func (c *RelayConn) read() {
	for {
		if c.channel.session.closed || c.closed {
			// 会话关闭
			// log.Println("relay tcp 关闭")
			break
		}

		// 一次读取一个数据包
		if c.conn != nil {
			header := make([]byte, 1)
			//n, e := c.conn.Read(header)
			n, e := io.ReadFull(c.conn, header)
			if e == nil && n == 1 {
				// 读取包id
				idData := make([]byte, 4)
				//n, e = c.conn.Read(idData)
				n, e = io.ReadFull(c.conn, idData)
				if e == nil && n == 4 {
					requestId := byteArrayToUin32(idData)

					// 读数据包长度4个字节
					lenData := make([]byte, 4)
					//n, e = c.conn.Read(lenData)
					n, e = io.ReadFull(c.conn, lenData)
					if e == nil && n == 4 {
						size := byteArrayToUin32(lenData)
						if size > 30720 {
							continue
						}

						// 读业务数据
						data := make([]byte, size)
						// n, e = c.conn.Read(data)
						n, e = io.ReadFull(c.conn, data)
						if e == nil && uint32(n) == size {
							go c.handler(header[0], requestId, data)
						} else {
							// log.Println("tcp连接有问题", e, "uint32(n)=", "size=", uint32(n), size)
							c.restart()
						}

					} else {
						// log.Println("tcp连接有问题", e, "n == 4=?", n == 4)
						c.restart()
					}

				} else {
					// log.Println("tcp连接有问题", e, " 2 -> n == 4=?", n == 4)
					c.restart()
				}

			} else {
				// log.Println("tcp连接有问题", e, " n == 1=?", n == 1)
				c.restart()
			}

		} else {
			time.Sleep(time.Millisecond * 200)
		}
	}
}

func (c *RelayConn) ok() {
	c.badCount = 0
	c.status = true
	c.channel.status = true
	c.channel.badCount = 0
	c.channel.session.status = true
	c.lastActive = Now()

	// 通知session会话已经通了(保证不阻塞)
	select {
	case c.channel.session.respChan <- []byte{1}:
		// 发送成功
	default:
		// 发送失败，处理逻辑（如丢弃数据、记录日志等）
	}
}

func (c *RelayConn) failure() {
	// 连续三次就重连
	c.badCount++
	c.channel.status = false
	if c.badCount >= 3 {
		c.restart()
	}
}

func (c *RelayConn) restart() {
	// log.Println("RelayConn 重制")

	c.badCount = 0
	c.status = false
	c.channel.status = false

	if c.conn != nil {
		c.conn.Close()
	}

	c.conn = nil
}

func (c *RelayConn) close() {
	if c.conn != nil {
		c.conn.Close()
	}
	c.status = false
	c.closed = true

	// 清除所有桶数据
	clearMap(c.response_pool)
}

// 处理业务
func (c *RelayConn) handler(header uint8, requestId uint32, data []byte) {
	// 解密
	dataM, e1 := aesUtil.DecryptBytes(data, c.channel.session.sessionPwd)
	if e1 != nil {
		// log.Println("解密错误:", e1)
		return
	}

	if header == 1 {

		// 2. 再解压
		//r, err := zlib.NewReader(bytes.NewReader(dataM))
		//if err != nil {
		//	return
		//}
		//defer r.Close()
		//dataM, _ = io.ReadAll(r)

		// 提取 业务类型
		if dataM[0] == 0 {
			// 回复确认包
			c.responseStatus(requestId)

			// 心跳->回复确认
		} else if dataM[0] == 1 {
			// 回复确认包
			c.responseStatus(requestId)

			// 文件数据
			p := newPackageByByte(dataM)
			if p != nil {
				// session 处理收到的分片
				go c.channel.session.handReadPackage(p)
			}
		} else if dataM[0] == 2 {
			// 回复确认包
			c.responseStatus(requestId)

			//log.Println("收到业务数据:", dataM)
			// 字节数据
			p := newPackageByByte(dataM)
			if p != nil {
				// session 处理收到的分片
				go c.channel.session.handReadPackage(p)
			}
		} else if dataM[0] == 3 {
			// 回复确认包
			c.responseStatus(requestId)

			// 对方通知关闭会话
			// log.Println("收到通知->关闭会话")
			time.Sleep(time.Second * 3)
			c.channel.session.close()
		} else if dataM[0] == 4 {
			// log.Println("内网连接请求")
			// 对方请求获取本地内网ip和端口好,想尝试内网直连 "ip:端口"
			param := c.channel.session.privateChannel.ip + ":" + c.channel.session.privateChannel.port
			c.responseData(requestId, []byte(param))
		} else if dataM[0] == 5 {
			p := newPackageByByte(dataM)
			if p != nil && p.index == 0 {
				// 对方p2p请求获取本地公网ip和端口好,想尝试内网直连 "公网ip:公网端口1:公网端口2"
				param := c.channel.session.p2pChannel.ip + ":" + c.channel.session.p2pChannel.port3 + ":" + c.channel.session.p2pChannel.port3
				c.responseData(requestId, []byte(param))
			}
		}

	} else if header == 0 {

		// --- 优化开始 ---
		// 尝试从 map 中找到对应的 channel
		if val, ok := c.response_pool.Load(requestId); ok {
			if ch, ok := val.(chan []byte); ok {
				c.channel.lastResponseTime = Now()

				// 尝试发送数据，不阻塞 read 线程
				// 使用 select 防止如果 request 已经超时退出，channel 被关闭或不可写导致 panic
				select {
				case ch <- dataM:
					// 发送成功，request 线程会被唤醒
				default:
					// 极端情况：channel 已满或已关闭，忽略
				}
			}
		}
		// --- 优化结束 ---
		// 注意：这里不再需要 Store 了

	}

}
