// ByteArrayStreamTest.java
// Tests java.io.ByteArrayOutputStream in-memory byte buffer.
// Expected output:
//   hi
//   2

import java.io.*;

class ByteArrayStreamTest {
    public static void main(String[] args) throws Exception {
        ByteArrayOutputStream baos = new ByteArrayOutputStream();
        baos.write(104);  // 'h'
        baos.write(105);  // 'i'
        System.out.println(baos.toString());
        System.out.println(baos.size());
    }
}
