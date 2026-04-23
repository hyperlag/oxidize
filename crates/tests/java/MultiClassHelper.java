// Stage 23: multiple top-level classes in one file.
// MathHelper is a package-private class; MultiClassHelper is the public
// entry-point that calls its static methods.
class MathHelper {
    static int square(int n) {
        return n * n;
    }

    static int cube(int n) {
        return n * n * n;
    }
}

public class MultiClassHelper {
    public static void main(String[] args) {
        System.out.println(MathHelper.square(4));
        System.out.println(MathHelper.cube(3));
    }
}
