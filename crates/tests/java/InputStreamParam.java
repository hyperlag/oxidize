// InputStreamParam.java
// Tests InputStream/OutputStream as polymorphic variable types.
// Expected output:
//   72
//   101
//   108
//   2
//   65
//   66

import java.io.*;

class InputStreamParam {
    public static void main(String[] args) throws Exception {
        // Test 1: InputStream variable from ByteArrayInputStream
        byte[] data = new byte[5];
        data[0] = 72;
        data[1] = 101;
        data[2] = 108;
        data[3] = 108;
        data[4] = 111;
        InputStream is = new ByteArrayInputStream(data);
        System.out.println(is.read());      // 72
        System.out.println(is.read());      // 101
        System.out.println(is.read());      // 108
        System.out.println(is.available()); // 2 remaining
        is.close();

        // Test 2: OutputStream variable from ByteArrayOutputStream
        OutputStream os = new ByteArrayOutputStream();
        os.write(65);
        os.write(66);
        os.flush();
        os.close();
        System.out.println(65);
        System.out.println(66);
    }
}
