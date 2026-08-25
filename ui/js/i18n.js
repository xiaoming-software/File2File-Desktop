(function () {
  var STORAGE_KEY = "file2file.lang";
  var listeners = [];
  var EN = {
    "安全直连，传文件、管网盘、控桌面": "Secure P2P for files, personal cloud, and desktop control",
    "点对点传输大文件": "Send large files peer to peer",
    "访问自己的个人网盘": "Open your personal cloud",
    "远程控制 Windows 电脑": "Remote-control a Windows PC",
    "无需公网 IP，数据不经过第三方网盘": "No public IP. Data never goes through a third-party cloud.",
    "登录": "Sign in",
    "登录中...": "Signing in...",
    "请输入 Token 与密码以连接 webrpc 网络": "Enter your token and password to join the webrpc network",
    "请输入 Token": "Enter token",
    "选择已保存的 Token": "Choose a saved token",
    "选择": "Choose",
    "密码": "Password",
    "请输入密码": "Enter password",
    "显示密码": "Show password",
    "隐藏密码": "Hide password",
    "显示": "Show",
    "隐藏": "Hide",
    "认证口令": "Passphrase",
    "请输入认证口令（可留空）": "Enter passphrase (optional)",
    "显示认证口令": "Show passphrase",
    "隐藏认证口令": "Hide passphrase",
    "保存 Token": "Remember token",
    "一键注册": "Quick register",
    "正在注册": "Registering",
    "正在创建 webrpc 账户，请稍候…": "Creating your webrpc account. Please wait…",
    "创建账户": "Create account",
    "登录控制台": "Sign in to console",
    "领取 Token": "Claim tokens",
    "正在创建 webrpc 账户…": "Creating your webrpc account…",
    "正在登录控制台…": "Signing in to the console…",
    "正在领取免费 Token…": "Claiming your free tokens…",
    "注册成功": "Registration complete",
    "注册成功！Token、密码与认证口令已填入下方登录框，请确认后登录。": "Done! Token, password, and passphrase are filled in the sign-in form below. Review them, then sign in.",
    "立即登录": "Sign in now",
    "稍后登录": "Sign in later",
    "管理 Token": "Manage tokens",
    "正在打开 webrpc 控制台…": "Opening webrpc console…",
    "无法打开控制台，请检查网络后重试": "Could not open the console. Check your network and try again.",
    "注册失败，请检查网络后重试": "Registration failed. Check your network and try again.",
    "注册超时，请稍后重试": "Registration timed out. Please try again later.",
    "本机一键注册次数已达上限（2 次）。请前往 webrpc.cn 自行注册。": "Quick register is limited to 2 times on this device. Please register at webrpc.cn.",
    "一键注册已达上限": "Quick register limit reached",
    "Token 尚未就绪，请稍后重试": "Tokens are not ready yet. Please try again later.",
    "Token 过期时间": "Token expires",
    "续费": "Renew",
    "Token 续费提醒": "Token renewal reminder",
    "您的 Token 将在 {date} 过期（剩余 {days} 天），请及时续费。": "Your token expires on {date} ({days} days left). Please renew soon.",
    "到期时间：{date}": "Expires: {date}",
    "是否前往 webrpc 控制台续费？": "Open the webrpc console to renew?",
    "前往续费": "Renew now",
    "稍后": "Later",
    "点击 {link} 获取 token": "Get a token at {link}",
    "登陆失败,请检查网络或者token与密码是否正确": "Sign-in failed. Check your network, token, and password.",
    "Token 和密码不能为空": "Token and password are required",
    "会话管理": "Sessions",
    "口令": "Passphrase",
    "未设置": "Not set",
    "登录时间": "Signed in",
    "登录状态": "Status",
    "已登录": "Signed in",
    "当前连接会话": "Active sessions",
    "退出登录": "Sign out",
    "聊天会话": "Chats",
    "新建会话": "New chat",
    "拖动调整聊天会话与网盘区域高度": "Drag to resize chats and cloud",
    "拖动调整高度": "Drag to resize",
    "网盘连接": "Cloud",
    "新建连接": "New connection",
    "未选择": "Nothing selected",
    "在左侧选择一个聊天会话或网盘，或点击「新建会话」「新建连接」。": "Select a chat or cloud on the left, or create a new one.",
    "请先建立 P2P 连接": "Connect over P2P first",
    "请先连接网盘": "Connect the cloud first",
    "当前会话尚未与对端连通，连接成功后即可收发消息和文件。": "This chat is not connected yet. After it connects, you can send messages and files.",
    "当前尚未与 NAS / 网盘设备连通。连接成功后即可查阅和更新家里的文件。": "This cloud device is not connected yet. After it connects, you can browse and update files.",
    "可留空，可修改": "Optional, can be changed later",
    "连接": "Connect",
    "连接中...": "Connecting...",
    "已连接": "Connected",
    "未连接": "Not connected",
    "连接失败。请确认对方 Token 是否在线，以及当前网络是否可达。": "Could not connect. Check that the peer token is online and this network can reach it.",
    "连接失败。请确认网盘 Token 是否在线，以及当前网络是否可达。": "Could not connect. Check that the cloud token is online and this network can reach it.",
    "上传": "Upload",
    "任务": "Tasks",
    "显示方式": "View",
    "列表": "List",
    "图标": "Icons",
    "向上": "Up",
    "当前路径": "Current path",
    "刷新": "Refresh",
    "搜索网盘": "Search cloud",
    "网盘概况": "Cloud overview",
    "总容量": "Total",
    "剩余": "Free",
    "已用": "Used",
    "文件": "Files",
    "正在获取网盘信息…": "Loading cloud info…",
    "名称": "Name",
    "种类": "Kind",
    "大小": "Size",
    "修改日期": "Modified",
    "松开即可上传到当前目录": "Drop to upload here",
    "松开即可发送文件": "Drop to send files",
    "清除": "Clear",
    "关闭任务": "Close tasks",
    "移除文件": "Remove file",
    "输入消息，Enter 发送，Shift+Enter 换行": "Enter to send, Shift+Enter for a new line",
    "语音通话": "Voice call",
    "语音": "Voice",
    "远程控制": "Remote control",
    "截图 (⌃⌘A)": "Screenshot (⌃⌘A)",
    "截图": "Screenshot",
    "更多截图选项": "More screenshot options",
    "隐藏当前窗口后截图": "Hide this window, then capture",
    "选择文件": "Choose file",
    "发送": "Send",
    "输入对方 Token 添加到聊天会话列表。认证口令可留空，连接时仍可修改。": "Enter the other person’s token to add a chat. Passphrase is optional and can be changed when you connect.",
    "对方 Token": "Peer token",
    "请输入对方 Token": "Enter peer token",
    "可留空": "Optional",
    "取消": "Cancel",
    "添加": "Add",
    "输入家里 NAS 或网盘设备的 webrpc Token，添加到「网盘连接」。连接后可查阅和更新 NAS 上的文件。认证口令可留空，连接时仍可修改。": "Enter the webrpc token for your NAS or cloud device. After connecting, you can browse and update files. Passphrase is optional and can be changed when you connect.",
    "网盘 Token": "Cloud token",
    "请输入网盘 Token": "Enter cloud token",
    "设置备注": "Rename",
    "设置网盘备注": "Rename cloud",
    "设置会话备注": "Rename chat",
    "账号备注": "Account label",
    "设置账号备注": "Set account label",
    "备注名称": "Display name",
    "例如：办公室 NAS": "e.g. Office NAS",
    "例如：家里 NAS": "e.g. Home NAS",
    "例如：小明": "e.g. Alex",
    "保存": "Save",
    "删除会话": "Delete chat",
    "删除连接": "Delete connection",
    "删除后将从列表和本地缓存中移除。": "This removes it from the list and local cache.",
    "删除后将从「网盘连接」列表和本地缓存中移除。不会影响聊天会话。": "This removes it from Cloud and local cache. Chats are not affected.",
    "新建文件": "New file",
    "新建文件夹": "New folder",
    "请填写文件名，必须包含扩展名，例如 readme.txt": "Enter a file name with an extension, e.g. readme.txt",
    "请填写文件夹名，不能包含扩展名（不要使用点号）。": "Enter a folder name without an extension (no dots).",
    "文件名": "File name",
    "文件夹名": "Folder name",
    "例如 文档": "e.g. Documents",
    "例如 readme.txt": "e.g. readme.txt",
    "确认": "OK",
    "移动到": "Move to",
    "将所选项目移动到下面打开的目录。可编辑路径后回车或点「打开」进入，双击文件夹进入子目录。点「确定」后移动到当前打开的目录。": "Move the selection into the folder below. Edit the path and press Enter or Open, or double-click a folder. OK moves into the folder currently open.",
    "打开": "Open",
    "确定": "OK",
    "正在压缩": "Compressing",
    "正在压缩文件夹…": "Compressing folder…",
    "后台进行": "Run in background",
    "关闭": "Close",
    "删除": "Delete",
    "下载": "Download",
    "移动": "Move",
    "重命名": "Rename",
    "压缩": "Compress",
    "新建": "New",
    "文件夹": "Folder",
    "文稿": "Document",
    "正在加载文件…": "Loading file…",
    "已使用系统软件打开。请在办公软件中保存，保存后会自动同步到网盘。": "Opened in your desktop app. Save there and it will sync to the cloud automatically.",
    "回到网盘": "Back to cloud",
    "立即同步": "Sync now",
    "文件已修改，结束编辑前是否同步到网盘？": "This file has changes. Sync to the cloud before closing?",
    "不同步": "Don't sync",
    "同步": "Sync",
    "正在编辑": "Editing",
    "文本": "Text",
    "退出": "Close",
    "文件已修改，是否保存？": "This file has changes. Save them?",
    "不保存": "Don't save",
    "静音": "Mute",
    "取消静音": "Unmute",
    "拒绝": "Decline",
    "接听": "Accept",
    "挂断": "Hang up",
    "全屏": "Full screen",
    "退出全屏": "Exit full screen",
    "结束": "End",
    "同意": "Allow",
    "等待对方桌面…": "Waiting for their desktop…",
    "拖到文件夹后松开即可移动": "Drop on a folder to move",
    "无法发送文件": "Can't send file",
    "请先选择并连接一个会话，再拖入文件发送。": "Select and connect a chat, then drop files to send.",
    "请先连接当前会话，再拖入文件发送。": "Connect this chat, then drop files to send.",
    "无法发送文件夹": "Can't send folder",
    "不支持直接发送文件夹。请打开文件夹，选中其中的文件后再拖入会话。": "Folders can't be sent directly. Open the folder and drop the files instead.",
    "拖入的内容不是可发送的文件，请重新选择后再试。": "That drop is not a sendable file. Choose files and try again.",
    "读取拖入的文件失败，请重新选择后再试。": "Couldn't read the dropped file. Choose it again.",
    "无法上传文件": "Can't upload file",
    "请先连接网盘，再拖入文件上传。": "Connect the cloud, then drop files to upload.",
    "无法上传文件夹": "Can't upload folder",
    "不支持直接上传文件夹。请打开文件夹后，选中其中的文件再拖入。": "Folders can't be uploaded directly. Open the folder and drop the files instead.",
    "拖入的内容不是可上传的文件，请重新选择后再试。": "That drop is not an uploadable file. Choose files and try again.",
    "无法截图": "Can't capture screenshot",
    "请先选择并连接一个会话，再使用截图。": "Select and connect a chat, then take a screenshot.",
    "请先连接当前会话，再使用截图。": "Connect this chat, then take a screenshot.",
    "截图失败，请稍后重试。": "Screenshot failed. Try again shortly.",
    "无法发送截图": "Can't send screenshot",
    "当前会话未连接，截图未发送。": "This chat is not connected. The screenshot was not sent.",
    "读取截图失败，请重新截取后再试。": "Couldn't read the screenshot. Capture it again.",
    "语音通话失败，请稍后重试。": "Voice call failed. Try again shortly.",
    "无法语音通话": "Can't start voice call",
    "请先选择并连接一个会话。": "Select and connect a chat first.",
    "请先连接当前会话，再发起语音通话。": "Connect this chat, then start a voice call.",
    "对方": "Peer",
    "正在呼叫": "Calling",
    "邀请你语音通话": "Incoming voice call",
    "语音通话中": "Voice call in progress",
    "远程控制失败，请稍后重试。": "Remote control failed. Try again shortly.",
    "无法远程控制": "Can't start remote control",
    "远程控制只支持电脑会话。": "Remote control is only available for computer chats.",
    "请先连接当前会话，再发起远程控制。": "Connect this chat, then start remote control.",
    "结束控制": "End control",
    "正在请求远程控制": "Requesting remote control",
    "请求远程控制你的电脑": "Remote control request",
    "{name} 想查看你的桌面": "{name} wants to view your desktop",
    "正在被远程控制": "Your computer is being controlled",
    "{name} 正在控制你的电脑，你也可以继续使用": "{name} is controlling your computer. You can keep using it.",
    "对方桌面": "Their desktop",
    "暂时不支持控制 Mac 桌面。": "Controlling a Mac desktop is not supported yet.",
    "暂无已保存的 Token": "No saved tokens",
    "暂无任务": "No tasks",
    "重试": "Retry",
    "进行中的任务不能删除": "A running task can't be deleted",
    "删除此任务": "Delete this task",
    " · 耗时 {elapsed} · 平均 {speed}": " · {elapsed} · avg {speed}",
    "排队中": "Queued",
    "进行中": "Running",
    "已完成": "Done",
    "已中断": "Stopped",
    "失败": "Failed",
    "松开即可上传到 {name}": "Drop to upload to {name}",
    "目标位置已存在同名文件或文件夹": "A file or folder with this name already exists there",
    "源文件不存在，可能已被删除或移动": "The source is missing. It may have been deleted or moved.",
    "不能移动到该位置": "Can't move to that location",
    "移动失败，请稍后重试": "Move failed. Try again shortly.",
    "移动失败，请稍后重试。": "Move failed. Try again shortly.",
    "移动超时，请稍后重试。": "Move timed out. Try again shortly.",
    "(未命名)": "(untitled)",
    "无法移动": "Can't move",
    "部分项目未能移动": "Some items were not moved",
    "以下项目没有移动成功：": "These items were not moved:",
    "知道了": "OK",
    "已在该目录，无需移动": "Already in that folder",
    "不能将文件夹移动到自身": "A folder can't be moved into itself",
    "不能将文件夹移动到自身或子目录中": "A folder can't be moved into itself or a subfolder",
    "{name} 等 {n} 项": "{name} and {n} items",
    "移动 {n} 项到「{name}」": "Move {n} items to “{name}”",
    "移动到「{name}」": "Move to “{name}”",
    "创建失败": "Couldn't create",
    "创建超时": "Create timed out",
    "无法移动到该位置": "Can't move there",
    "正在加载目录…": "Loading folder…",
    "当前目录没有文件夹，可以直接确定以移动到这里。": "No subfolders here. You can OK to move into this folder.",
    "无法加载目录": "Couldn't load folder",
    "目录列表请求超时": "Folder listing timed out",
    "将已选的 {n} 个项目移动到下面打开的目录。可编辑路径后回车或点「打开」进入，双击文件夹进入子目录。点「确定」后移动到当前打开的目录。": "Move {n} selected items into the folder below. Edit the path and press Enter or Open, or double-click a folder. OK moves into the folder currently open.",
    "将「{name}」移动到下面打开的目录。可编辑路径后回车或点「打开」进入，双击文件夹进入子目录。点「确定」后移动到当前打开的目录。": "Move “{name}” into the folder below. Edit the path and press Enter or Open, or double-click a folder. OK moves into the folder currently open.",
    "未选择要移动的项目": "Nothing selected to move",
    "请填写文件夹名": "Enter a folder name",
    "请填写文件名": "Enter a file name",
    "请填写名称": "Enter a name",
    "名称不能包含特殊字符": "The name can't contain special characters",
    "名称不能以点开头": "The name can't start with a dot",
    "文件必须填写扩展名，例如 readme.txt": "The file name needs an extension, e.g. readme.txt",
    "文件夹不能包含扩展名，请不要使用点号": "A folder name can't include an extension or dots",
    "已存在同名文件或文件夹": "A file or folder with this name already exists",
    "名称不合法": "That name isn't valid",
    "确定删除文件夹「{name}」？文件夹会连同其中的内容一起删除。删除后无法恢复。": "Delete folder “{name}”? The folder and everything inside it will be removed. This can't be undone.",
    "确定删除文件「{name}」？删除后无法恢复。": "Delete file “{name}”? This can't be undone.",
    "确定删除已选的 {n} 个项目？{extra}删除后无法恢复。": "Delete {n} selected items? {extra}This can't be undone.",
    "其中的文件夹会连同内容一起删除。": "Folders will be removed with their contents. ",
    "文件或文件夹不存在": "The file or folder doesn't exist",
    "不能删除该项": "That item can't be deleted",
    "删除失败，请稍后重试": "Delete failed. Try again shortly.",
    "无法删除": "Can't delete",
    "部分项目未能删除": "Some items were not deleted",
    "以下项目没有删除成功：": "These items were not deleted:",
    "删除项目": "Delete items",
    "删除文件夹": "Delete folder",
    "删除文件": "Delete file",
    "搜索失败": "Search failed",
    "搜索超时": "Search timed out",
    "正在搜索…": "Searching…",
    "没有匹配的文件或文件夹": "No matching files or folders",
    "仅显示部分结果，请再输入更完整的名称": "Showing partial results. Type a more complete name.",
    "已存在同名压缩文件，无法继续压缩": "A zip with this name already exists",
    "文件夹不存在": "The folder doesn't exist",
    "不能压缩该项": "That item can't be compressed",
    "压缩失败，请稍后重试": "Compress failed. Try again shortly.",
    "已存在": "Already exists",
    "正在压缩「{name}」，请稍候…": "Compressing “{name}”…",
    "当前已有压缩任务进行中，请等待完成后再试。": "A compress task is already running. Wait for it to finish.",
    "压缩完成": "Compressed",
    "已生成「{name}」。": "Created “{name}”.",
    "无法压缩": "Can't compress",
    "「{name}」没有压缩成功。": "“{name}” was not compressed.",
    "「{name}」{reason}": "“{name}” {reason}",
    "/文档/工作": "/Documents/Work",
    "文件列表请求超时": "File listing timed out",
    "图片过大请下载到本地查看": "This image is too large. Download it to view.",
    "正在加载预览…": "Loading preview…",
    "预览超时": "Preview timed out",
    "文件过大无法在线编辑": "This file is too large to edit here",
    "无法打开，请稍后重试": "Couldn't open. Try again shortly.",
    "打开超时": "Open timed out",
    "正在下载文件…": "Downloading…",
    "无法预览该音频": "This audio can't be previewed",
    "无法预览该视频": "This video can't be previewed",
    "音频过大请下载到本地查看": "This audio is too large. Download it to play.",
    "视频过大请下载到本地查看": "This video is too large. Download it to play.",
    "无法预览，请稍后重试": "Preview failed. Try again shortly.",
    "有文件正在传输，请稍候": "A transfer is in progress. Try again shortly.",
    "等待下载完成后再打开": "Wait for the download to finish, then open it",
    "下载中": "Downloading",
    "同步中": "Syncing",
    "未安装办公软件": "No office app",
    "有未同步": "Unsynced",
    "已同步": "Synced",
    "打开中": "Opening",
    "结束编辑": "Stop editing",
    "正在同步到网盘…": "Syncing to cloud…",
    "已保存到本地，正在等待自动同步": "Saved locally, waiting to auto-sync",
    "未检测到 WPS、Office 或 LibreOffice，请先安装办公软件后再打开。": "No WPS, Office, or LibreOffice found. Install an office app, then open again.",
    "正在同步，请稍候再关闭": "Syncing. Wait before closing.",
    "已保存到本地，正在自动同步": "Saved locally, auto-syncing",
    "已保存到本地，等待写入完成": "Saved locally, waiting for the write to finish",
    "网盘未连接": "Cloud is not connected",
    "正在打开文件…": "Opening file…",
    "请安装办公软件": "Install an office app",
    "文件还在加载": "The file is still loading",
    "没有修改": "No changes",
    "同步失败": "Sync failed",
    "同步超时，将重试": "Sync timed out. Retrying.",
    "文件正在写入，稍后自动同步": "File is still being written. It will sync shortly.",
    "网盘忙，稍后自动同步": "Cloud is busy. It will sync shortly.",
    "该文件不是可编辑的文本": "This file isn't editable text",
    "保存失败": "Save failed",
    "有传输任务进行中，请稍候再保存": "A transfer is running. Save again shortly.",
    "正在保存…": "Saving…",
    "保存超时": "Save timed out",
    "已保存，还有未提交的修改": "Saved, with more unsaved edits",
    "已保存": "Saved",
    "图像": "Image",
    "影片": "Video",
    "音频": "Audio",
    "PDF 文稿": "PDF",
    "压缩包": "Archive",
    "代码": "Code",
    "正在加载": "Loading",
    "正在获取当前目录的文件列表…": "Loading this folder…",
    "无法加载": "Couldn't load",
    "此文件夹为空": "This folder is empty",
    "当前目录没有可见的文件或文件夹。": "There are no visible files or folders here.",
    "更新于 {time}": "Updated {time}",
    "暂无聊天会话<br />点击「新建会话」连接对端": "No chats yet<br />Click New chat to connect",
    "关闭会话": "Close chat",
    "暂无连接<br />点击「新建连接」连接家里的 NAS": "No clouds yet<br />Click New connection to add a NAS",
    "关闭连接": "Close connection",
    "未备注": "No name",
    "清空内容": "Clear history",
    "设置": "Settings",
    "网盘 Token：": "Cloud token: ",
    "对方 Token：": "Peer token: ",
    "复制全文": "Copy all",
    "复制": "Copy",
    "已复制": "Copied",
    "重新发送": "Resend",
    "重发": "Resend",
    "消息正在发送中，请稍后再删除。": "This message is still sending. Delete it later.",
    "文件正在发送中，请等待发送完成后再删除这条记录。": "This file is still sending. Wait until it finishes, then delete the record.",
    "文件正在接收中，请等待接收完成后再删除这条记录。": "This file is still receiving. Wait until it finishes, then delete the record.",
    "删除这条消息": "Delete this message",
    "确定删除这条消息吗？删除后无法从会话中恢复。": "Delete this message? It can't be restored in this chat.",
    "确定删除这条发送记录吗？只会从会话中移除这条气泡，不会删除你电脑上的原始文件。": "Delete this sent record? Only the bubble is removed. The original file on your computer stays.",
    "确定删除这条接收记录吗？会话中的气泡和本地缓存文件都会被删除，删除后无法恢复。": "Delete this received record? The bubble and the local cache file will be removed. This can't be undone.",
    "打开所在目录": "Show in folder",
    "查看图片": "View image",
    "已发送": "Sent",
    "已接收": "Received",
    "发送成功": "Sent",
    "发送失败": "Failed to send",
    "发送中": "Sending",
    "接收成功": "Received",
    "接收中": "Receiving",
    "接收失败": "Failed to receive",
    "会话通信异常，通知消息未能送达，连接已关闭。请检查网络后重试。": "The session signal didn't go through, so the connection was closed. Check the network and try again.",
    "网盘已断开连接，请重新连接。": "The cloud disconnected. Connect again.",
    "对端已断开连接，请重新连接。": "The peer disconnected. Connect again.",
    "将清空本会话在本地的聊天记录（含文本和文件预览）。不会断开连接，也不会删除会话、备注或 Token。": "This clears the local history for this chat, including text and file previews. It does not disconnect, and it does not delete the chat, name, or token.",
    "清空": "Clear",
    "清除任务记录": "Clear task history",
    "将清除已完成、失败和已中断的任务记录。进行中和排队中的任务不会被清除。": "This clears finished, failed, and stopped tasks. Running and queued tasks stay.",
    "该会话已存在": "This chat already exists",
    "保存会话失败": "Couldn't save chat",
    "该网盘已存在": "This cloud already exists",
    "保存网盘失败": "Couldn't save cloud",
    "语言": "Language",
    "耗时": "Elapsed",
    " · 平均 {speed}": " · avg {speed}",
    "拖动鼠标选择区域 · Esc 取消": "Drag to select an area · Esc to cancel",
    "细": "Thin",
    "中": "Medium",
    "粗": "Thick",
    "矩形": "Rectangle",
    "椭圆": "Ellipse",
    "箭头": "Arrow",
    "画笔": "Pen",
    "马赛克": "Mosaic",
    "文字": "Text",
    "撤销": "Undo",
    "完成": "Done",
  };

  function detectSystemLang() {
    var list = navigator.languages && navigator.languages.length ? navigator.languages : [navigator.language || ""];
    var i;
    for (i = 0; i < list.length; i += 1) {
      if (String(list[i] || "").toLowerCase().indexOf("zh") === 0) return "zh";
    }
    return "en";
  }

  function readSavedLang() {
    try {
      var saved = window.localStorage.getItem(STORAGE_KEY);
      if (saved === "zh" || saved === "en") return saved;
    } catch (err) {}
    return "";
  }

  var current = readSavedLang() || detectSystemLang();

  function interpolate(text, vars) {
    if (!vars) return text;
    return String(text).replace(/\{(\w+)\}/g, function (_, key) {
      return vars[key] == null ? "" : String(vars[key]);
    });
  }

  function t(zh, vars) {
    if (zh == null) return "";
    var src = String(zh);
    var out = current === "en" && Object.prototype.hasOwnProperty.call(EN, src) ? EN[src] : src;
    return interpolate(out, vars);
  }

  function getLang() {
    return current;
  }

  function applyDom(root) {
    var scope = root || document;
    scope.querySelectorAll("[data-i18n]").forEach(function (el) {
      el.textContent = t(el.getAttribute("data-i18n"));
    });
    scope.querySelectorAll("[data-i18n-html]").forEach(function (el) {
      el.innerHTML = t(el.getAttribute("data-i18n-html"), {
        link: '<a href="https://webrpc.cn" target="_blank" rel="noopener noreferrer">https://webrpc.cn</a>',
      });
    });
    scope.querySelectorAll("[data-i18n-placeholder]").forEach(function (el) {
      el.setAttribute("placeholder", t(el.getAttribute("data-i18n-placeholder")));
    });
    scope.querySelectorAll("[data-i18n-title]").forEach(function (el) {
      el.setAttribute("title", t(el.getAttribute("data-i18n-title")));
    });
    scope.querySelectorAll("[data-i18n-aria]").forEach(function (el) {
      el.setAttribute("aria-label", t(el.getAttribute("data-i18n-aria")));
    });
    document.documentElement.lang = current === "zh" ? "zh-CN" : "en";
    document.documentElement.setAttribute("data-lang", current);
    document.querySelectorAll(".lang-switch-btn").forEach(function (btn) {
      var on = btn.getAttribute("data-lang") === current;
      btn.classList.toggle("is-on", on);
      btn.setAttribute("aria-pressed", on ? "true" : "false");
    });
    var groups = document.querySelectorAll(".lang-switch");
    groups.forEach(function (group) {
      group.setAttribute("aria-label", t("语言"));
    });
  }

  function setLang(lang, persist) {
    current = lang === "en" ? "en" : "zh";
    if (persist !== false) {
      try {
        window.localStorage.setItem(STORAGE_KEY, current);
      } catch (err) {}
    }
    applyDom();
    listeners.forEach(function (fn) {
      try {
        fn(current);
      } catch (err) {}
    });
  }

  function onChange(fn) {
    if (typeof fn === "function") listeners.push(fn);
  }

  document.addEventListener("click", function (event) {
    var btn = event.target.closest(".lang-switch-btn");
    if (!btn) return;
    event.preventDefault();
    setLang(btn.getAttribute("data-lang"));
  });

  window.F2F_I18N = {
    t: t,
    getLang: getLang,
    setLang: setLang,
    applyDom: applyDom,
    onChange: onChange,
  };

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", function () {
      applyDom();
    });
  } else {
    applyDom();
  }
})();
