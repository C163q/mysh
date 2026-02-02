# 开发笔记

记录开发过程、思路等。

## 内建命令

一个最简单的Shell在处理命令时，无非就是3情况：

- 内建命令
- 外部可执行程序
- 无法访问

在处理内建命令时，我的第一个想法就是设置一个注册表，一般来说是一个哈希表，键为命令名，值为调用的函数。
这样子就可以方便的集中管理。

内建命令既然是固定的，在编译时就确定了，自然就会想到可以把这张表作为一个`const`的全局变量。然而可惜的是，
在目前版本的rust中，无论是`BTreeMap`还是`HashMap`，都不是`const`的。准确来说，我需要用到`new`和`insert`
方法，但前者只有`new`是`const`的，而后者都不是`const`的。

这就很难受，因为`static`声明周期的变量必须是能够`const`初始化的，使用`OnceLock`就太过了，Shell程序毕竟
还是单线程的程序，因此使用`thread_local`就可以了。

这样，整个命令的处理逻辑就非常清晰了。首先查表，如果找到命令，调用键值存储的函数，没有找到，尝试调用作为
外部程序路径运行，如果找不到，则表示无法找到命令。

## 在PATH中查找可执行文件

无论是`execv`还是`Command::new`，都会自动帮助寻找`PATH`当中的可执行文件，但是内建命令`type`就不一样了，
我们必须自己实现查找的逻辑。

最初的实现逻辑嵌套非常深，非常不优雅，好在的是rust的`clippy`足够强大，在不少地方都表示可以简化，比如：

```rust
if let Some(v) = value {
    if u.condition() {}
}
// ^^^ old / new vvv
if let Some(v) = value && u.condition() {}
```

其实最最重要的改动还是使用了迭代器的`flatten()`方法。rust的`Option`和`Result`可以转换为迭代器，并且只在
`Some(_)`和`Ok(_)`的时候产出一个元素的技巧实在太好用了，配合`flatten`，可以很方便的处理迭代目录中的文件时，
返回的`Result`。

总之，在反复的优化之后，代码变成如下的样子：

```rust
fn get_executable_in_path(cmd: &str, env: &ExecEnv) -> Option<DirEntry> {
    fn dir_get_executable(name: &str, reader: ReadDir) -> Option<DirEntry> {
        reader
            .flatten()
            .find(|entry| entry.path().is_executable() && entry.file_name() == name)
    }

    for dir in env.path_env.iter() {
        if let Ok(entries) = read_dir(dir)
            && let Some(entry) = dir_get_executable(cmd, entries)
        {
            return Some(entry);
        }
    }

    None
}
```

优雅。

## 重定向

我没有去查其他的Shell是如何实现重定向的，我的方法是交换两个文件的文件描述符。既然使用的是这种方法，那么
自然就不支持`Windows`系统了。

比如我有这样的重定向的要求：`3> file`。依照交换文件描述符的方法，首先打开`file`文件，得到其文件描述符(`new_fd`)，
然后使用`dup`复制`fd == 3`(`old_fd`)的文件`tmp_fd`，然后使用`dup2`使用`new_fd`覆盖`old_fd`，然后使用`tmp_fd`
覆盖`new_fd`，最后关闭`tmp_fd`（因为此时`tmp_fd`和`new_fd`都指向原本`fd == 3`的文件）。

在启动进程之后，换回去，其实就是将`new_fd`覆盖`old_fd`，然后关闭`new_fd`。

上面这种方法其实可以通过`RAII`来实现，进程启动前换`fd`的操作由对象的构造函数处理，启动之后换回的操作使用
对象的析构函数处理。

## 命令解析

命令解析是这里面写的最最头疼的部分，最主要的原因是由相当多的`edge case`。比如`echo "value" >1.txt`该怎么处理？
`echo 'va'lu"e"   >>> 1.txt`该怎么处理？等等……

我暂时不想在此处讲太多，因为这只是多种特殊情况的堆砌而已。

## 集成测试

我发现rust的测试好像是会截获`println!`的输出的，我在`cargo test`中如法炮制，交换fd，想要捕获指令输出到
标准输出的结果，结果发现不行……

在查了`println!`的实现后发现，它会调用`stdio::print_to`函数，这个函数内部会使用`print_to_buffer_if_capture_used`
函数，它会将内容捕获，而不输出。

要么解决方法之一是使用`cargo run -- --nocapture`。但是……我觉得还是应该重写一下输出部分，防止捕获。
