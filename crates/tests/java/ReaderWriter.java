// ReaderWriter.java
// Tests Reader/Writer as polymorphic types and BufferedReader(StringReader).
// Expected output:
//   hello
//   world
//   done

import java.io.*;

class ReaderWriter {
    public static void main(String[] args) throws Exception {
        // Test 1: BufferedReader wrapping StringReader
        BufferedReader br = new BufferedReader(new StringReader("hello\nworld"));
        System.out.println(br.readLine());  // hello
        System.out.println(br.readLine());  // world
        br.close();

        System.out.println("done");
    }
}
