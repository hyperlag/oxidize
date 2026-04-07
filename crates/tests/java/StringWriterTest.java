// StringWriterTest.java
// Tests java.io.StringWriter + PrintWriter integration.
// Expected output:
//   hello world
//   foo bar

import java.io.*;

class StringWriterTest {
    public static void main(String[] args) throws Exception {
        // StringWriter accumulates text; PrintWriter writes into it
        StringWriter sw1 = new StringWriter();
        PrintWriter pw1 = new PrintWriter(sw1);
        pw1.print("hello");
        pw1.print(" world");
        pw1.flush();
        System.out.println(sw1.toString());

        // Second independent StringWriter
        StringWriter sw2 = new StringWriter();
        PrintWriter pw2 = new PrintWriter(sw2);
        pw2.print("foo");
        pw2.print(" bar");
        pw2.flush();
        System.out.println(sw2.toString());
    }
}
