// @ts-check
///<reference path="../global.d.ts"/>

describe('Validates Get Summary', () => {
    beforeEach(function () {
      cy.loginToAuth0(import.meta.env.VITE_AUTH0_USERNAME, import.meta.env.VITE_AUTH0_PASSWORD);
    });
  
    it('Checks get_summary loads with a 200', () => {
  
      // Go to the Synthetic Workspace
      cy.visit(import.meta.env.VITE_SI_WORKSPACE_URL + '/w/' + import.meta.env.VITE_SI_WORKSPACE_ID + '/head')

      cy.intercept('GET', import.meta.env.VITE_SI_WORKSPACE_URL + '/api/qualification/get_summary?visibility_change_set_pk=00000000000000000000000000', (req) => {
        // Log the intercepted request URL and response status code
        cy.log(`Request to ${req.url}`, req.response.statusCode);
        // Assert that the status code is 200
        expect(req.response.statusCode).to.eq(200);
      });

    })
})