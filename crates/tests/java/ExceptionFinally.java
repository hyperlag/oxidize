public class ExceptionFinally {
    public static void main(String[] args) {
        try {
            throw new RuntimeException("oops");
        } catch (RuntimeException e) {
            System.out.println("caught: " + e.getMessage());
        } finally {
            System.out.println("finally");
        }
        System.out.println("after");
    }
}
