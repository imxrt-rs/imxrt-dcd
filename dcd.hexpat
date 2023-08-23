// ImHex <https://imhex.werwolv.net/> Pattern Definition for parsing DCD binary.

#pragma endian big
#pragma eval_depth 100

enum CommandType : u8 {
    Nop = 0xC0,
    Write = 0xCC,
    Check = 0xCF,
    Dcd = 0xD2,
};

struct Header {
    CommandType type;
    u16 len;
};

enum WriteOp : u8 {
    Write = 0b00,
    WriteAlt = 0b10,
    Clear = 0b01,
    Set = 0b11,
};

bitfield WriteFlags {
    padding: 3;
    WriteOp op : 2;
    width : 3;
};

struct AddrValue {
    u32 addr;
    u32 value;
};

struct Write<auto len> {
    WriteFlags flags;
    AddrValue addr_value[while($ < addressof(this) + len)];
};

enum CheckCond : u8 {
    AllClear = 0b00,
    AllSet = 0b10,
    AnyClear = 0b01,
    AnySet = 0b11,
};

bitfield CheckFlags {
    padding: 3;
    CheckCond cond : 2;
    width : 3;
};

struct Check<auto len> {
    CheckFlags flags;
    u32 addr;
    u32 value;
    if ($ < addressof(this) + len) {
        u32 count;
    }
};

struct Command : Header {
    if (type == CommandType::Write) {
        Write<len - 3> write;
    } else if (type == CommandType::Check) {
        Check<len - 3> check;
    } else if (type == CommandType::Nop) {
        u8 nop[len - 3];
    }
};

struct Dcd : Header {
    u8 ver;
    Command commands[while($ < addressof(this) + len)];
};

Dcd dcd @ 0;